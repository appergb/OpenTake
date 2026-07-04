//! A minimal XML document tree shared by the XML-based exporters
//! ([`crate::fcpxml_modern`]). Builders describe document *structure*;
//! [`render_document`] owns all indentation and escaping, so no emitter
//! hardcodes whitespace. (The legacy XMEML exporter in [`crate::fcpxml`] keeps
//! its own equivalent private copy to avoid churning that tested module.)

/// One XML element. A leaf carries `text` (`<n>text</n>`); a branch carries
/// `children`; an element with neither renders self-closing (`<n/>`).
pub struct XmlNode {
    name: String,
    attributes: Vec<(String, String)>,
    text: Option<String>,
    children: Vec<XmlNode>,
}

impl XmlNode {
    /// Append children and return self (chaining after `with_owned_attrs`).
    pub fn with_children(mut self, children: Vec<XmlNode>) -> Self {
        self.children = children;
        self
    }

    /// Build an element from owned `(key, value)` attribute pairs (for callers
    /// that compute attribute strings dynamically).
    pub fn with_owned_attrs(name: &str, attrs: Vec<(String, String)>) -> Self {
        XmlNode {
            name: name.to_string(),
            attributes: attrs,
            text: None,
            children: Vec::new(),
        }
    }
}

/// A branch element with children and no attributes.
pub fn el(name: &str, children: Vec<XmlNode>) -> XmlNode {
    XmlNode {
        name: name.to_string(),
        attributes: Vec::new(),
        text: None,
        children,
    }
}

/// A branch element with borrowed `(key, value)` attributes and children.
pub fn el_attrs(name: &str, attrs: Vec<(&str, &str)>, children: Vec<XmlNode>) -> XmlNode {
    XmlNode {
        name: name.to_string(),
        attributes: attrs
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        text: None,
        children,
    }
}

/// A leaf element `<name>value</name>` (value is XML-escaped on render).
pub fn leaf_text(name: &str, value: &str) -> XmlNode {
    XmlNode {
        name: name.to_string(),
        attributes: Vec::new(),
        text: Some(value.to_string()),
        children: Vec::new(),
    }
}

/// A `name="value"` attribute pair where the value is a boolean rendered as
/// `1`/`0` (FCPXML's boolean attribute convention). Exposed for exporters that
/// build attribute lists directly.
pub fn boolean_attr(name: &str, value: bool) -> (String, String) {
    (name.to_string(), if value { "1" } else { "0" }.to_string())
}

/// Render a full document: the XML declaration, an optional DOCTYPE line, then
/// the root element tree. Two-space indentation per level.
pub fn render_document(doctype: &str, root: &XmlNode) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{doctype}\n{}",
        render(root, 0)
    )
}

fn render(node: &XmlNode, indent: usize) -> String {
    let pad = " ".repeat(indent);
    let attrs: String = node
        .attributes
        .iter()
        .map(|(k, v)| format!(" {k}=\"{}\"", escape_xml(v)))
        .collect();
    if let Some(text) = &node.text {
        return format!(
            "{pad}<{}{attrs}>{}</{}>",
            node.name,
            escape_xml(text),
            node.name
        );
    }
    if node.children.is_empty() {
        return format!("{pad}<{}{attrs}/>", node.name);
    }
    let inner: Vec<String> = node
        .children
        .iter()
        .map(|c| render(c, indent + 2))
        .collect();
    format!(
        "{pad}<{}{attrs}>\n{}\n{pad}</{}>",
        node.name,
        inner.join("\n"),
        node.name
    )
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_leaf() {
        assert_eq!(render(&leaf_text("name", "hi"), 0), "<name>hi</name>");
    }

    #[test]
    fn renders_self_closing_for_empty() {
        let n = XmlNode::with_owned_attrs("asset", vec![("id".into(), "r1".into())]);
        assert_eq!(render(&n, 0), "<asset id=\"r1\"/>");
    }

    #[test]
    fn renders_nested_two_space_indent() {
        let n = el("a", vec![el("b", vec![leaf_text("c", "x")])]);
        assert_eq!(render(&n, 0), "<a>\n  <b>\n    <c>x</c>\n  </b>\n</a>");
    }

    #[test]
    fn escapes_text_and_attrs() {
        let n = XmlNode::with_owned_attrs("e", vec![("k".into(), "<v>".into())])
            .with_children(vec![leaf_text("c", "a&b<c>")]);
        let out = render(&n, 0);
        assert!(out.contains("k=\"&lt;v&gt;\""));
        assert!(out.contains("a&amp;b&lt;c&gt;"));
    }

    #[test]
    fn boolean_attr_is_one_zero() {
        assert_eq!(
            boolean_attr("hasVideo", true),
            ("hasVideo".into(), "1".into())
        );
        assert_eq!(
            boolean_attr("hasAudio", false),
            ("hasAudio".into(), "0".into())
        );
    }

    #[test]
    fn document_has_declaration_and_doctype() {
        let doc = render_document("<!DOCTYPE fcpxml>", &el("fcpxml", vec![]));
        assert!(doc.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE fcpxml>\n"));
    }

    #[test]
    fn with_children_replaces_children() {
        let n = XmlNode::with_owned_attrs("p", vec![]).with_children(vec![leaf_text("c", "v")]);
        assert_eq!(render(&n, 0), "<p>\n  <c>v</c>\n</p>");
    }
}
