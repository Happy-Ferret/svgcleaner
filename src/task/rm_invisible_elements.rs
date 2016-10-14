/****************************************************************************
**
** svgcleaner could help you to clean up your SVG files
** from unnecessary data.
** Copyright (C) 2012-2016 Evgeniy Reizner
**
** This program is free software; you can redistribute it and/or modify
** it under the terms of the GNU General Public License as published by
** the Free Software Foundation; either version 2 of the License, or
** (at your option) any later version.
**
** This program is distributed in the hope that it will be useful,
** but WITHOUT ANY WARRANTY; without even the implied warranty of
** MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
** GNU General Public License for more details.
**
** You should have received a copy of the GNU General Public License along
** with this program; if not, write to the Free Software Foundation, Inc.,
** 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301 USA.
**
****************************************************************************/

use super::short::{EId, AId};

use svgdom::{Document, Node, AttributeValue, ValueId};
use svgdom::types::FuzzyEq;

// TODO: process mask element
// TODO: process visibility
// TODO: process feGaussianBlur with stdDeviation=0
// TODO: split to suboptions

pub fn remove_invisible_elements(doc: &Document) {
    let mut is_any_removed = false;
    process_display_attribute(doc, &mut is_any_removed);
    process_paths(doc, &mut is_any_removed);
    process_clip_paths(doc, &mut is_any_removed);
    process_empty_filter(doc, &mut is_any_removed);
    process_fe_color_matrix(doc);
    process_use(doc, &mut is_any_removed);
    process_gradients(doc);
    process_rect(doc, &mut is_any_removed);

    if is_any_removed {
        super::remove_unused_defs(doc);
    }
}

// Remove invalid elements from 'clipPath' and if 'clipPath' is empty or became empty
// - remove it and all elements that became invalid or unused.
fn process_clip_paths(doc: &Document, is_any_removed: &mut bool) {
    let mut nodes = Vec::with_capacity(16);
    let mut clip_paths = Vec::with_capacity(16);

    // remove all invalid children
    for node in doc.descendants().filter(|n| n.is_tag_id(EId::ClipPath)) {
        for child in node.children() {
            if !is_valid_clip_path_elem(&child) {
                nodes.push(child.clone());
            }
        }

        while let Some(n) = nodes.pop() {
            n.remove();
        }

        if !node.has_children() {
            clip_paths.push(node.clone());
        }
    }

    if !clip_paths.is_empty() {
        *is_any_removed = true;
    }

    // Remove empty clipPath's.
    // Note, that all elements that uses this clip path also became invisible,
    // so we can remove them as well.
    while let Some(n) = clip_paths.pop() {
        for link in n.linked_nodes() {
            link.remove();
        }
        n.remove();
    }
}

fn is_valid_clip_path_elem(node: &Node) -> bool {
    // https://www.w3.org/TR/SVG/masking.html#EstablishingANewClippingPath

    fn is_valid_shape(node: &Node) -> bool {
           node.is_basic_shape()
        || node.is_tag_id(EId::Path)
        || node.is_tag_id(EId::Text)
    }

    if node.is_tag_id(EId::Use) {
        if let Some(av) = node.attribute_value(AId::XlinkHref) {
            if let AttributeValue::Link(link) = av {
                return is_valid_shape(&link);
            }
        }
    }

    is_valid_shape(node)
}

// Paths with empty 'd' attribute are invisible and we can remove them.
fn process_paths(doc: &Document, is_any_removed: &mut bool) {
    let mut paths = Vec::with_capacity(16);

    fn is_invisible(node: &Node) -> bool {
        if node.has_attribute(AId::D) {
            let attrs = node.attributes();
            match *attrs.get_value(AId::D).unwrap() {
                AttributeValue::Path(ref d) => {
                    if d.d.is_empty() {
                        return true;
                    }
                }
                // invalid value type
                _ => return true,
            }
        } else {
            // not set
            return true;
        }

        false
    }

    for node in doc.descendants().filter(|n| n.is_tag_id(EId::Path)) {
        if is_invisible(&node) {
            paths.push(node.clone());
        }
    }

    if !paths.is_empty() {
        *is_any_removed = true;
    }

    for n in paths {
        n.remove();
    }
}

// Remove elements with 'display:none'.
fn process_display_attribute(doc: &Document, is_any_removed: &mut bool) {
    let mut nodes = Vec::with_capacity(16);

    let mut iter = doc.descendants();
    while let Some(node) = iter.next() {
        // if elements has attribute 'display:none' and this element is not used - we can remove it
        if node.has_attribute_with_value(AId::Display, ValueId::None) && !node.is_used() {
            // all children must be unused to
            if !node.descendants().any(|n| n.is_used()) {
                // TODO: ungroup used elements and remove unused
                nodes.push(node.clone());

                if node.has_children() {
                    iter.skip_children();
                }
            }
        }
    }

    if !nodes.is_empty() {
        *is_any_removed = true;
    }

    for n in nodes {
        n.remove();
    }
}

// remove 'filter' elements without children
fn process_empty_filter(doc: &Document, is_any_removed: &mut bool) {
    let nodes: Vec<Node> = doc.descendants()
                              .filter(|n| n.is_tag_id(EId::Filter) && !n.has_children())
                              .collect();

    // Note, that all elements that uses this filter also became invisible,
    // so we can remove them as well.
    for n in nodes {
        *is_any_removed = true;
        for link in n.linked_nodes() {
            link.remove();
        }
        n.remove();
    }
}

// remove feColorMatrix with default values
fn process_fe_color_matrix(doc: &Document) {
    let mut nodes = Vec::with_capacity(16);

    for node in doc.descendants().filter(|n| n.is_tag_id(EId::Filter)) {
        if node.children().count() != 1 {
            continue;
        }

        let child = node.children().nth(0).unwrap();

        if !child.is_tag_id(EId::FeColorMatrix) {
            continue;
        }

        let attrs = child.attributes();

        // It's a very simple implementation since we do not parse matrix,
        // but it's enough to remove default feColorMatrix generated by Illustrator.
        if let Some(&AttributeValue::String(ref t)) = attrs.get_value(AId::Type) {
            if t == "matrix" {
                if let Some(&AttributeValue::String(ref values)) = attrs.get_value(AId::Values) {
                    if values == "1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 1 0" {
                        // we remove the whole 'filter' elements
                        nodes.push(node);
                    }
                }
            }
        }
    }

    for n in nodes {
        n.remove();
    }
}

// 'use' element without 'xlink:href' attribute is pointless
fn process_use(doc: &Document, is_any_removed: &mut bool) {
    let nodes: Vec<Node> = doc.descendants()
                              .filter(|n| n.is_tag_id(EId::Use) && !n.has_attribute(AId::XlinkHref))
                              .collect();

    for n in nodes {
        *is_any_removed = true;
        n.remove();
    }
}

fn process_gradients(doc: &Document) {
    let mut nodes = Vec::with_capacity(16);

    {
        // gradient without children and link to other gradient is pointless
        let iter = doc.descendants()
                      .filter(|n| super::is_gradient(n))
                      .filter(|n| !n.has_children() && !n.has_attribute(AId::XlinkHref));

        for n in iter {
            for link in n.linked_nodes() {
                while let Some(aid) = find_link_attribute(&link, &n) {
                    link.set_attribute(aid, ValueId::None);
                }
            }
            nodes.push(n.clone());
        }
    }

    {
        // 'If one stop is defined, then paint with the solid color fill using the color
        // defined for that gradient stop.'
        let iter = doc.descendants()
                      .filter(|n| super::is_gradient(n))
                      .filter(|n| n.children().count() == 1 && !n.has_attribute(AId::XlinkHref));

        for n in iter {
            let stop = n.children().nth(0).unwrap();
            // unwrap is safe, because we already resolved all 'stop' attributes
            let color = *stop.attribute_value(AId::StopColor).unwrap().as_color().unwrap();
            let opacity = *stop.attribute_value(AId::StopOpacity).unwrap().as_number().unwrap();

            for link in n.linked_nodes() {
                while let Some(aid) = find_link_attribute(&link, &n) {
                    link.set_attribute(aid, color);
                    if opacity.fuzzy_ne(&1.0) {
                        match aid {
                            AId::Fill => link.set_attribute(AId::FillOpacity, opacity),
                            AId::Stroke => link.set_attribute(AId::StrokeOpacity, opacity),
                            _ => {}
                        }
                    }
                }
            }
            nodes.push(n.clone());
        }
    }

    for n in nodes {
        n.remove();
    }
}

fn find_link_attribute(node: &Node, link: &Node) -> Option<AId> {
    let attrs = node.attributes();

    for (aid, attr) in attrs.iter_svg() {
        match attr.value {
            AttributeValue::Link(ref n) | AttributeValue::FuncLink(ref n) => {
                if *n == *link {
                    return Some(aid);
                }
            }
            _ => {}
        }
    }

    None
}

// remove rect's with zero size
fn process_rect(doc: &Document, is_any_removed: &mut bool) {
    let mut nodes = Vec::with_capacity(16);

    for n in doc.descendants().filter(|n| n.is_tag_id(EId::Rect)) {
        let attrs = n.attributes();
        if    attrs.get_value(AId::Width).unwrap().as_length().unwrap().num == 0.0
           || attrs.get_value(AId::Height).unwrap().as_length().unwrap().num == 0.0 {
            nodes.push(n.clone());
        }
    }

    for n in nodes {
        *is_any_removed = true;
        n.remove();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use svgdom::{Document, WriteToString};
    use task::{group_defs, final_fixes};

    macro_rules! test {
        ($name:ident, $in_text:expr, $out_text:expr) => (
            #[test]
            fn $name() {
                let doc = Document::from_data($in_text).unwrap();
                // we must prepare defs, because `remove_invisible_elements`
                // invokes `remove_unused_defs`
                group_defs(&doc);
                remove_invisible_elements(&doc);
                // removes `defs` element
                final_fixes(&doc);
                assert_eq_text!(doc.to_string_with_opt(&write_opt_for_tests!()), $out_text);
            }
        )
    }

    macro_rules! test_eq {
        ($name:ident, $in_text:expr) => (
            test!($name, $in_text, String::from_utf8_lossy($in_text));
        )
    }

    test!(rm_clip_path_children_1,
b"<svg>
    <defs>
        <clipPath>
            <g/>
            <rect height='5' width='5'/>
        </clipPath>
    </defs>
</svg>",
"<svg>
    <defs>
        <clipPath>
            <rect height='5' width='5'/>
        </clipPath>
    </defs>
</svg>
");

    test!(rm_clip_path_children_2,
b"<svg>
    <defs>
        <clipPath>
            <use/>
            <use xlink:href='#g1'/>
            <use xlink:href='#rect1'/>
        </clipPath>
    </defs>
    <rect id='rect1' height='5' width='5'/>
    <g id='g1'/>
</svg>",
"<svg>
    <defs>
        <clipPath>
            <use xlink:href='#rect1'/>
        </clipPath>
    </defs>
    <rect id='rect1' height='5' width='5'/>
    <g id='g1'/>
</svg>
");

    test!(rm_clip_path_1,
b"<svg>
    <clipPath id='cp1'/>
    <rect clip-path='url(#cp1)' height='5' width='5'/>
    <rect clip-path='url(#cp1)' height='5' width='5'/>
</svg>",
"<svg/>
");

    test!(rm_clip_path_2,
b"<svg>
    <linearGradient id='lg1'/>
    <clipPath id='cp1'/>
    <rect clip-path='url(#cp1)' fill='url(#lg1)' height='5' width='5'/>
</svg>",
"<svg/>
");

    test!(rm_clip_path_3,
b"<svg>
    <clipPath>
        <rect display='none' height='5' width='5'/>
    </clipPath>
</svg>",
"<svg/>
");

    test!(rm_path_1,
b"<svg>
    <path/>
</svg>",
"<svg/>
");

    test!(rm_path_2,
b"<svg>
    <path d=''/>
</svg>",
"<svg/>
");

    test!(rm_path_3,
b"<svg>
    <linearGradient id='lg1'/>
    <path d='' fill='url(#lg1)'/>
</svg>",
"<svg/>
");

    test!(rm_display_none_1,
b"<svg>
    <path display='none'/>
</svg>",
"<svg/>
");

    test!(rm_display_none_2,
b"<svg>
    <g display='none'>
        <rect height='5' width='5'/>
    </g>
</svg>",
"<svg/>
");

    test_eq!(skip_display_none_1,
b"<svg>
    <g display='none'>
        <rect id='r1' height='5' width='5'/>
    </g>
    <use xlink:href='#r1'/>
</svg>
");

    test!(rm_filter_1,
b"<svg>
    <filter/>
</svg>",
"<svg/>
");

    test!(rm_filter_2,
b"<svg>
    <filter id='f1'/>
    <rect filter='url(#f1)' height='5' width='5'/>
</svg>",
"<svg/>
");

    test!(rm_use_1,
b"<svg>
    <use/>
</svg>",
"<svg/>
");

    test!(rm_gradient_1,
b"<svg>
    <linearGradient id='lg1'/>
    <rect fill='url(#lg1)' height='5' width='5'/>
    <rect stroke='url(#lg1)' height='5' width='5'/>
</svg>",
"<svg>
    <rect fill='none' height='5' width='5'/>
    <rect height='5' stroke='none' width='5'/>
</svg>
");

    test!(rm_gradient_2,
b"<svg>
    <linearGradient id='lg1'>
        <stop offset='0.5' stop-color='#ff0000' stop-opacity='0.5'/>
    </linearGradient>
    <rect fill='url(#lg1)' stroke='url(#lg1)' height='5' width='5'/>
</svg>",
"<svg>
    <rect fill='#ff0000' fill-opacity='0.5' height='5' stroke='#ff0000' stroke-opacity='0.5' width='5'/>
</svg>
");

    test!(rm_rect_1,
b"<svg>
    <rect width='0' height='0'/>
    <rect width='0' height='0'/>
    <rect width='0' height='0'/>
</svg>",
"<svg/>
");

    test!(rm_fe_color_matrix_1,
b"<svg>
    <filter id='filter1'>
        <feColorMatrix type='matrix' values='1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 1 0'/>
    </filter>
    <rect filter='url(#filter1)' height='10' width='10'/>
</svg>",
"<svg>
    <rect height='10' width='10'/>
</svg>
");

}
