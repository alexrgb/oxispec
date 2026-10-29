//! A static site from a graph: one page per node, linked by ID, so a link
//! never goes to a path that a rename can break. This module builds the
//! pages as text. Reading git revisions and writing files is done by the
//! command line.
//!
//! Every link on a page stays inside the same build, which is one revision,
//! so a reader never ends up on a different version by following a link.

use std::collections::{BTreeMap, HashMap};

use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, TagEnd, html};

use crate::check::{LinkResolution, resolve_link};
use crate::diff::{Change, diff_nodes};
use crate::graph::Graph;
use crate::id;
use crate::node::Node;

/// The style sheet every page links to.
pub const STYLE: &str = include_str!("site.css");

/// A page of the site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// Relative to the folder of this version, for example `REQ-AUTH-0001.html`.
    pub path: String,
    pub html: String,
}

/// A version of the specs that readers can switch to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionLink {
    /// The folder, next to this one, that holds the pages of that version.
    pub dir: String,
    pub label: String,
}

/// Which earlier version pages are compared with.
pub struct Previous<'a> {
    pub label: String,
    pub graph: &'a Graph,
}

pub struct SiteOptions<'a> {
    pub title: String,
    /// What this build shows, for example `working tree` or `v0.2.0 (3f2a1c9)`.
    pub revision: String,
    /// This version's folder name, when there are several versions.
    pub current: Option<String>,
    /// Other versions to switch to, newest first. Includes this one.
    pub versions: Vec<VersionLink>,
    pub previous: Option<Previous<'a>>,
}

/// A file name for an ID: letters, digits, `-`, `_` and `.`, nothing else.
pub fn page_name(id: &str) -> String {
    let safe: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{safe}.html")
}

pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

fn title_of(node: &Node) -> String {
    node.title
        .clone()
        .or_else(|| node.requirements.first().map(|r| r.name.clone()))
        .unwrap_or_else(|| node.id.clone())
}

/// Links that cannot run script or load something unexpected.
fn safe_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    match lower.split_once(':') {
        None => true,
        Some((scheme, _)) => {
            // `a/b:c` has a slash before the colon, so it is a path.
            scheme.contains(['/', '?', '#']) || matches!(scheme, "http" | "https" | "mailto")
        }
    }
}

/// The part of `text` that is an ID of a node in the graph, if the word is one.
fn known_id<'a>(graph: &Graph, word: &'a str) -> Option<&'a str> {
    let word = word.trim_end_matches('.');
    let parsed = id::parse(word)?;
    parsed.namespace.is_none().then_some(())?;
    graph.get(parsed.base)?;
    Some(word)
}

/// Turns IDs that name a node into links, in a piece of text.
fn link_ids(graph: &Graph, text: &str, out: &mut Vec<Event<'static>>) {
    let mut plain = String::new();
    let flush = |plain: &mut String, out: &mut Vec<Event<'static>>| {
        if !plain.is_empty() {
            out.push(Event::Text(CowStr::from(std::mem::take(plain))));
        }
    };
    let is_word = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_');
    let mut rest = text;
    while !rest.is_empty() {
        let start = rest.find(is_word).unwrap_or(rest.len());
        plain.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest.find(|c: char| !is_word(c)).unwrap_or(rest.len());
        let word = &rest[..end];
        rest = &rest[end..];
        match known_id(graph, word) {
            Some(found) => {
                flush(&mut plain, out);
                let target = id::parse(found).map_or(found, |p| p.base);
                let anchor = id::parse(found)
                    .and_then(|p| p.child)
                    .map_or(String::new(), |c| format!("#{c}"));
                out.push(Event::Html(CowStr::from(format!(
                    "<a href=\"{}{anchor}\">",
                    escape(&page_name(target))
                ))));
                out.push(Event::Text(CowStr::from(found.to_string())));
                out.push(Event::Html(CowStr::from("</a>")));
                plain.push_str(&word[found.len()..]);
            }
            None => plain.push_str(word),
        }
    }
    flush(&mut plain, out);
}

/// Markdown to HTML. Raw HTML in the source is shown as text, links that
/// could run script are dropped, and IDs of known nodes become links.
pub fn markdown(graph: &Graph, source: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES;
    let mut events: Vec<Event<'static>> = Vec::new();
    let (mut in_code, mut in_link) = (false, 0usize);
    for event in Parser::new_ext(source, options) {
        match event {
            Event::Html(raw) | Event::InlineHtml(raw) => {
                events.push(Event::Text(CowStr::from(raw.to_string())));
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                in_code = true;
                events.push(Event::Start(Tag::CodeBlock(kind.into_static())));
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code = false;
                events.push(Event::End(TagEnd::CodeBlock));
            }
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                in_link += 1;
                let dest = if safe_url(&dest_url) {
                    dest_url.to_string()
                } else {
                    String::new()
                };
                events.push(Event::Start(Tag::Link {
                    link_type,
                    dest_url: CowStr::from(dest),
                    title: CowStr::from(title.to_string()),
                    id: CowStr::from(id.to_string()),
                }));
            }
            Event::End(TagEnd::Link) => {
                in_link = in_link.saturating_sub(1);
                events.push(Event::End(TagEnd::Link));
            }
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let dest = if safe_url(&dest_url) {
                    dest_url.to_string()
                } else {
                    String::new()
                };
                events.push(Event::Start(Tag::Image {
                    link_type,
                    dest_url: CowStr::from(dest),
                    title: CowStr::from(title.to_string()),
                    id: CowStr::from(id.to_string()),
                }));
            }
            Event::Text(text) if !in_code && in_link == 0 => {
                link_ids(graph, &text, &mut events);
            }
            other => events.push(other.into_static()),
        }
    }
    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    out
}

/// Link to a target of a node's `links`, resolved against the graph.
fn link_html(graph: &Graph, target: &str) -> String {
    let shown = escape(target);
    match resolve_link(graph, target) {
        LinkResolution::Ok => {
            let parsed = id::parse(target);
            let base = parsed.as_ref().map_or(target, |p| p.base);
            let anchor = parsed
                .and_then(|p| p.child)
                .map_or(String::new(), |c| format!("#{c}"));
            format!(
                "<a href=\"{}{anchor}\">{shown}</a>",
                escape(&page_name(base))
            )
        }
        LinkResolution::External => format!("<span class=\"external\">{shown}</span>"),
        _ => format!("<span class=\"missing\" title=\"no such node\">{shown}</span>"),
    }
}

fn badge(class: &str, text: &str) -> String {
    format!("<span class=\"badge {class}\">{}</span>", escape(text))
}

fn status_class(status: &str) -> String {
    let safe: String = status
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    format!("status-{safe}")
}

fn picker(options: &SiteOptions, target: &str) -> String {
    if options.versions.len() < 2 {
        return String::new();
    }
    let mut out = String::from("<label class=\"picker\">Version <select data-page=\"");
    out.push_str(&escape(target));
    out.push_str("\">");
    for version in &options.versions {
        let selected = options.current.as_deref() == Some(version.dir.as_str());
        out.push_str(&format!(
            "<option value=\"{}\"{}>{}</option>",
            escape(&version.dir),
            if selected { " selected" } else { "" },
            escape(&version.label)
        ));
    }
    out.push_str("</select></label>");
    out
}

const PICKER_SCRIPT: &str = "<script>document.querySelectorAll('.picker select').forEach(function(s){s.addEventListener('change',function(){var d=s.value,p=s.dataset.page,t='../'+d+'/'+p,f='../'+d+'/index.html';fetch(t,{method:'HEAD'}).then(function(r){location.href=r.ok?t:f}).catch(function(){location.href=t})})});</script>";

fn layout(options: &SiteOptions, heading: &str, target: &str, body: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{} · {}</title><link rel=\"stylesheet\" href=\"style.css\"></head><body><header><a class=\"home\" href=\"index.html\">{}</a><span class=\"revision\">{}</span>{}</header><main>{body}</main>{PICKER_SCRIPT}</body></html>\n",
        escape(heading),
        escape(&options.title),
        escape(&options.title),
        escape(&options.revision),
        picker(options, target)
    )
}

/// Who links to whom, computed once: base ID to the nodes that link to it.
fn reverse_links(graph: &Graph) -> HashMap<&str, Vec<(&Node, &str)>> {
    let mut map: HashMap<&str, Vec<(&Node, &str)>> = HashMap::new();
    for node in &graph.nodes {
        for (kind, targets) in &node.links {
            for target in targets {
                map.entry(id::base_of(target))
                    .or_default()
                    .push((node, kind.as_str()));
            }
        }
    }
    map
}

fn node_page(
    graph: &Graph,
    options: &SiteOptions,
    node: &Node,
    incoming: &HashMap<&str, Vec<(&Node, &str)>>,
    text_of: &dyn Fn(&Node) -> Option<String>,
) -> String {
    let title = title_of(node);
    let mut body = format!(
        "<h1><span class=\"id\">{}</span> {}</h1>",
        escape(&node.id),
        escape(&title)
    );

    body.push_str("<p class=\"meta\">");
    body.push_str(&badge("type", &node.ty));
    if let Some(status) = &node.status {
        body.push_str(&badge(&status_class(status), status));
    }
    let mut facts: Vec<(&str, String)> = Vec::new();
    if let Some(owner) = &node.owner {
        facts.push(("owner", owner.clone()));
    }
    if let Some(by) = &node.approved_by {
        let on = node
            .approved_on
            .as_ref()
            .map_or(String::new(), |d| format!(" on {d}"));
        facts.push(("approved by", format!("{by}{on}")));
    }
    if let Some(version) = &node.version {
        facts.push(("version", version.clone()));
    }
    facts.push(("file", node.path.display().to_string().replace('\\', "/")));
    for (name, value) in &facts {
        body.push_str(&format!(
            " <span class=\"fact\">{name}: {}</span>",
            escape(value)
        ));
    }
    body.push_str("</p>");

    if let Some(previous) = &options.previous {
        let changes = match previous.graph.get(&node.id) {
            None => vec!["new in this version".to_string()],
            Some(before) => diff_nodes(before, node)
                .iter()
                .map(Change::describe)
                .collect(),
        };
        if !changes.is_empty() {
            body.push_str(&format!(
                "<section class=\"changes\"><h2>Since {}</h2><ul>",
                escape(&previous.label)
            ));
            for change in changes {
                body.push_str(&format!("<li>{}</li>", escape(&change)));
            }
            body.push_str("</ul></section>");
        }
    }

    if let Some(text) = text_of(node) {
        let text = crate::parse::split_frontmatter(&text)
            .map_or(text.clone(), |(_, body, _)| body.to_string());
        body.push_str(&format!("<article>{}</article>", markdown(graph, &text)));
    }

    if !node.links.is_empty() {
        body.push_str("<section class=\"links\"><h2>Links</h2><ul>");
        for (kind, targets) in &node.links {
            for target in targets {
                body.push_str(&format!(
                    "<li><span class=\"kind\">{}</span> {}</li>",
                    escape(kind),
                    link_html(graph, target)
                ));
            }
        }
        body.push_str("</ul></section>");
    }

    if let Some(from) = incoming.get(node.id.as_str()) {
        body.push_str("<section class=\"links\"><h2>What links here</h2><ul>");
        for (other, kind) in from {
            body.push_str(&format!(
                "<li><a href=\"{}\">{}</a> <span class=\"kind\">{}</span> {}</li>",
                escape(&page_name(&other.id)),
                escape(&other.id),
                escape(kind),
                escape(&title_of(other))
            ));
        }
        body.push_str("</ul></section>");
    }
    layout(
        options,
        &format!("{} {}", node.id, title),
        &page_name(&node.id),
        &body,
    )
}

fn index_page(graph: &Graph, options: &SiteOptions) -> String {
    let mut by_type: BTreeMap<&str, Vec<&Node>> = BTreeMap::new();
    for node in &graph.nodes {
        by_type.entry(node.ty.as_str()).or_default().push(node);
    }
    let mut body = format!(
        "<h1>{}</h1><p class=\"lead\">{} nodes · {}</p><input id=\"filter\" type=\"search\" placeholder=\"Filter by ID, title or status\" aria-label=\"Filter\">",
        escape(&options.title),
        graph.nodes.len(),
        escape(&options.revision)
    );
    for (ty, nodes) in &by_type {
        body.push_str(&format!(
            "<section class=\"group\"><h2>{} <span class=\"count\">{}</span></h2><table><tbody>",
            escape(ty),
            nodes.len()
        ));
        for node in nodes {
            let status = node.status.as_deref().unwrap_or("");
            body.push_str(&format!(
                "<tr><td><a href=\"{}\">{}</a></td><td>{}</td><td>{}</td></tr>",
                escape(&page_name(&node.id)),
                escape(&node.id),
                escape(&title_of(node)),
                if status.is_empty() {
                    String::new()
                } else {
                    badge(&status_class(status), status)
                }
            ));
        }
        body.push_str("</tbody></table></section>");
    }
    body.push_str("<script>document.getElementById('filter').addEventListener('input',function(e){var q=e.target.value.toLowerCase();document.querySelectorAll('.group tr').forEach(function(r){r.hidden=q&&r.textContent.toLowerCase().indexOf(q)<0});document.querySelectorAll('.group').forEach(function(g){g.hidden=q&&!g.querySelector('tr:not([hidden])')})})</script>");
    layout(options, "Index", "index.html", &body)
}

/// All pages of one version. `text_of` gives the text of a node's file, from
/// disk or from a git revision. Pages of nodes it has no text for show only
/// metadata and links.
pub fn render(
    graph: &Graph,
    options: &SiteOptions,
    text_of: &dyn Fn(&Node) -> Option<String>,
) -> Vec<Page> {
    let incoming = reverse_links(graph);
    let mut pages = vec![Page {
        path: "index.html".into(),
        html: index_page(graph, options),
    }];
    let mut seen = std::collections::HashSet::new();
    for node in &graph.nodes {
        let path = page_name(&node.id);
        // A duplicate ID is an error `check` reports. Its first node gets the page.
        if !seen.insert(path.clone()) || path == "index.html" {
            continue;
        }
        pages.push(Page {
            path,
            html: node_page(graph, options, node, &incoming, text_of),
        });
    }
    pages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_file_text;
    use std::path::Path;

    fn graph(files: &[(&str, &str)]) -> Graph {
        Graph::from_nodes(
            files
                .iter()
                .filter_map(|(p, t)| parse_file_text(Path::new(p), t).0)
                .collect(),
        )
    }

    fn no_text(_: &Node) -> Option<String> {
        None
    }

    fn options() -> SiteOptions<'static> {
        SiteOptions {
            title: "Demo".into(),
            revision: "working tree".into(),
            current: None,
            versions: Vec::new(),
            previous: None,
        }
    }

    #[test]
    fn page_names_are_safe_file_names() {
        assert_eq!(page_name("REQ-AUTH-0001"), "REQ-AUTH-0001.html");
        assert_eq!(page_name("ns:REQ/../x"), "ns_REQ_.._x.html");
        assert_eq!(page_name("a b"), "a_b.html");
    }

    #[test]
    fn markdown_shows_raw_html_as_text_and_drops_script_links() {
        let g = graph(&[]);
        let html = markdown(
            &g,
            "<script>alert(1)</script>\n\n[bad](javascript:alert(1)) [ok](https://example.com) [rel](a/b:c) ![i](data:image/png;base64,AAAA)\n\n<img src=x onerror=alert(1)>\n",
        );
        assert!(!html.contains("<script>"), "{html}");
        assert!(!html.contains("<img src=x"), "{html}");
        assert!(
            html.contains("&lt;img src=x onerror=alert(1)&gt;"),
            "{html}"
        );
        assert!(html.contains("&lt;script&gt;"), "{html}");
        assert!(!html.contains("javascript:"), "{html}");
        assert!(html.contains("href=\"https://example.com\""), "{html}");
        assert!(html.contains("href=\"a/b:c\""), "{html}");
        assert!(!html.contains("data:image"), "{html}");
    }

    #[test]
    fn ids_in_text_become_links_but_not_in_code_or_existing_links() {
        let g = graph(&[(
            "a.md",
            "---\nid: REQ-A-0001\ntype: requirement\n---\n### Requirement: X\nIt SHALL.\n#### Scenario: S {#S1}\n",
        )]);
        let html = markdown(
            &g,
            "See REQ-A-0001, REQ-A-0001.S1 and REQ-NOPE-0001.\n\n`REQ-A-0001` and [REQ-A-0001](http://x)\n\n```\nREQ-A-0001\n```\n",
        );
        assert!(
            html.contains("<a href=\"REQ-A-0001.html\">REQ-A-0001</a>,"),
            "{html}"
        );
        assert!(
            html.contains("<a href=\"REQ-A-0001.html#S1\">REQ-A-0001.S1</a>"),
            "{html}"
        );
        assert!(html.contains("REQ-NOPE-0001."), "{html}");
        assert!(!html.contains("REQ-NOPE-0001.html"));
        assert!(html.contains("<code>REQ-A-0001</code>"), "{html}");
        assert_eq!(html.matches("<a ").count(), 3, "{html}");
    }

    #[test]
    fn scenario_headings_get_anchors() {
        let g = graph(&[]);
        let html = markdown(&g, "#### Scenario: Idle admin {#S1}\n");
        assert!(
            html.contains("<h4 id=\"S1\">Scenario: Idle admin</h4>"),
            "{html}"
        );
    }

    const REQ: &str = "---\nid: REQ-A-0001\ntype: requirement\nstatus: draft\nlinks:\n  decided-by: ADR-0001\n  refines: REQ-NOPE-0001\n  verifies: other:X-1\n---\n### Requirement: Timeout\nThe system SHALL end <b>idle</b> sessions.\n";
    const ADR: &str = "---\nid: ADR-0001\ntype: adr\n---\n# ADR-0001: Use Postgres\n";

    #[test]
    fn pages_link_by_id_in_both_directions() {
        let g = graph(&[("a.md", REQ), ("b.md", ADR)]);
        let pages = render(&g, &options(), &no_text);
        let names: Vec<&str> = pages.iter().map(|p| p.path.as_str()).collect();
        assert_eq!(names, ["index.html", "ADR-0001.html", "REQ-A-0001.html"]);

        let req = &pages[2].html;
        assert!(
            req.contains("<a href=\"ADR-0001.html\">ADR-0001</a>"),
            "{req}"
        );
        assert!(req.contains("class=\"missing\""), "{req}");
        assert!(req.contains("class=\"external\">other:X-1"), "{req}");
        assert!(req.contains("badge status-draft"), "{req}");
        let adr = &pages[1].html;
        assert!(adr.contains("What links here"), "{adr}");
        assert!(adr.contains("<a href=\"REQ-A-0001.html\">REQ-A-0001</a> <span class=\"kind\">decided-by</span> Timeout"), "{adr}");
        let index = &pages[0].html;
        assert!(index.contains("2 nodes"), "{index}");
        assert!(index.contains("<h2>requirement <span"), "{index}");
    }

    #[test]
    fn titles_and_text_are_escaped() {
        let g = graph(&[(
            "a.md",
            "---\nid: ADR-0002\ntype: adr\n---\n# ADR-0002: <script>x</script> & more\n",
        )]);
        let mut opts = options();
        opts.title = "A <b>\"project\"".into();
        let pages = render(&g, &opts, &no_text);
        for page in &pages {
            assert!(!page.html.contains("<script>x"), "{}", page.html);
            assert!(!page.html.contains("<b>\"project\""), "{}", page.html);
        }
        assert!(
            pages[1]
                .html
                .contains("&lt;script&gt;x&lt;/script&gt; &amp; more")
        );
    }

    #[test]
    fn a_picker_and_changes_appear_when_there_are_versions() {
        let before = graph(&[(
            "a.md",
            "---\nid: REQ-A-0001\ntype: requirement\nstatus: draft\n---\n### Requirement: Timeout\nThe system SHALL work.\n",
        )]);
        let after = graph(&[("a.md", REQ), ("b.md", ADR)]);
        let mut opts = options();
        opts.current = Some("v2".into());
        opts.versions = vec![
            VersionLink {
                dir: "latest".into(),
                label: "latest".into(),
            },
            VersionLink {
                dir: "v2".into(),
                label: "v2".into(),
            },
        ];
        opts.previous = Some(Previous {
            label: "v1".into(),
            graph: &before,
        });
        let pages = render(&after, &opts, &no_text);
        let adr = &pages[1].html;
        assert!(
            adr.contains("<h2>Since v1</h2><ul><li>new in this version</li>"),
            "{adr}"
        );
        let req = &pages[2].html;
        assert!(req.contains("link added: decided-by ADR-0001"), "{req}");
        assert!(
            req.contains("<option value=\"v2\" selected>v2</option>"),
            "{req}"
        );
        assert!(req.contains("data-page=\"REQ-A-0001.html\""), "{req}");
        // Without a second version there is nothing to pick.
        assert!(
            !render(&after, &options(), &no_text)[2]
                .html
                .contains("<select")
        );
    }

    #[test]
    fn node_text_is_rendered_when_it_is_available() {
        let g = graph(&[("a.md", REQ)]);
        let pages = render(&g, &options(), &|_| Some(REQ.to_string()));
        let html = &pages[1].html;
        assert!(html.contains("<article>"), "{html}");
        assert!(html.contains("&lt;b&gt;idle&lt;/b&gt;"), "{html}");
        assert!(!pages[1].html.contains("<b>idle"));
        assert!(
            !render(&g, &options(), &no_text)[1]
                .html
                .contains("<article>")
        );
    }
}
