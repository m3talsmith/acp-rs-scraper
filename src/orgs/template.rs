//! Organization templates. A small substitution language, not a program.
//!
//! The default page asks for Cinzel and Lora and then a generic serif. Those
//! font files are not shipped. The title and the motto come from the brand.

use super::{Audience, Org, get_org, list_posts, now, public_row};
use crate::prelude::*;

/// The white-label page. Headers ask for Cinzel, body text asks for Lora.
pub const DEFAULT_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>{{brand_title}}</title>
<style>
:root { {{theme_css}} }
h1, h2, h3 { font-family: Cinzel, serif; }
body { font-family: Lora, serif; background: var(--bg); color: var(--fg); }
a { color: var(--accent); }
.motto { color: var(--muted); }
</style>
</head>
<body>
<header>
<h1>{{brand_title}}</h1>
<p class="motto">{{brand_motto}}</p>
</header>
<h2>{{org_name}}</h2>
<p>{{org_description}}</p>
{{#posts}}
<article>
<h3><a href="{{permalink}}">{{title}}</a></h3>
<p>{{body}}</p>
</article>
{{/posts}}
{{#assets}}
<p><a href="{{download}}">{{name}}</a></p>
{{/assets}}
</body>
</html>
"#;

/// Colours the template receives as CSS variables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemePaint {
    /// Page background.
    pub bg: String,
    /// Body text.
    pub fg: String,
    /// Links.
    pub accent: String,
    /// The motto.
    pub muted: String,
}

impl ThemePaint {
    /// A neutral paint used when no theme file is open.
    pub fn neutral() -> Self {
        Self { bg: "#111111".into(), fg: "#eeeeee".into(), accent: "#2fb49a".into(), muted: "#88aaaa".into() }
    }

    /// CSS custom properties. The template drops this into `:root`.
    pub fn css(&self) -> String {
        format!("--bg: {}; --fg: {}; --accent: {}; --muted: {};", self.bg, self.fg, self.accent, self.muted)
    }
}

/// One post the template may print. [`render`] escapes the fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostSlot {
    /// Title.
    pub title: String,
    /// Body.
    pub body: String,
    /// Relative page, `posts/<id>.html`.
    pub permalink: String,
}

/// One asset the template may print.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetSlot {
    /// File name.
    pub name: String,
    /// Relative download URL.
    pub download: String,
}

/// What a render substitutes. The caller decides which rows are visible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    /// Organization display name.
    pub org_name: String,
    /// Organization description.
    pub org_description: String,
    /// Brand title. Not a hard-coded product string.
    pub brand_title: String,
    /// Brand motto. Empty when the brand has none.
    pub brand_motto: String,
    /// CSS variables from the theme on screen.
    pub theme_css: String,
    /// Posts the audience may see.
    pub posts: Vec<PostSlot>,
    /// Assets the audience may see.
    pub assets: Vec<AssetSlot>,
}

/// The stored template, or the default when the organization has not set one.
pub fn source(c: &Connection, org_id: i64) -> Result<String> {
    if !super::has_table(c, "org_templates") {
        return Ok(DEFAULT_TEMPLATE.to_string());
    }
    let stored: Option<String> = c
        .query_row("SELECT html FROM org_templates WHERE org_id = ?1", params![org_id], |r| r.get(0))
        .optional()?;
    Ok(stored.unwrap_or_else(|| DEFAULT_TEMPLATE.to_string()))
}

/// Replace the template. `html` is stored as text and is never executed.
pub fn set_source(c: &Connection, org_id: i64, html: &str) -> Result<()> {
    let html = check_template(html)?;
    let t = now();
    c.execute(
        "INSERT INTO org_templates (org_id, name, html, updated_at) VALUES (?1, 'default', ?2, ?3)
         ON CONFLICT(org_id) DO UPDATE SET html = excluded.html, updated_at = excluded.updated_at",
        params![org_id, html, t],
    )?;
    Ok(())
}

/// Forget a custom template. The next read is the default again.
pub fn reset_source(c: &Connection, org_id: i64) -> Result<()> {
    if super::has_table(c, "org_templates") {
        c.execute("DELETE FROM org_templates WHERE org_id = ?1", params![org_id])?;
    }
    Ok(())
}

/// Render `template` for `page`. Unknown tokens are left in place.
pub fn render(template: &str, page: &Page) -> Result<String> {
    check_template(template)?;
    let with_posts = expand(template, "posts", &page.posts, |body, post| {
        fill(body, &[("title", &post.title), ("body", &post.body), ("permalink", &post.permalink)])
    })?;
    let with_assets = expand(&with_posts, "assets", &page.assets, |body, asset| {
        fill(body, &[("name", &asset.name), ("download", &asset.download)])
    })?;
    Ok(fill(
        &with_assets,
        &[
            ("org_name", &page.org_name),
            ("org_description", &page.org_description),
            ("brand_title", &page.brand_title),
            ("brand_motto", &page.brand_motto),
        ],
    )
    .replace("{{theme_css}}", &page.theme_css))
}

/// The page a render of this organization would use.
pub fn page_for(c: &Connection, slug: &str, brand_title: &str, brand_motto: &str, theme_css: &str, audience: Audience) -> Result<Page> {
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    page_of(c, &org, brand_title, brand_motto, theme_css, audience)
}

fn page_of(c: &Connection, org: &Org, brand_title: &str, brand_motto: &str, theme_css: &str, audience: Audience) -> Result<Page> {
    let mut posts = Vec::new();
    for post in list_posts(c, org.id)? {
        let show = match audience {
            Audience::Owner => true,
            Audience::Public => public_row(org, &post, false),
        };
        if !show {
            continue;
        }
        let mut title = post.title.clone();
        if post.archived {
            title = format!("[archived] {title}");
        }
        posts.push(PostSlot { title, body: post.body.clone(), permalink: format!("posts/{}.html", post.id) });
    }
    let mut assets = Vec::new();
    for asset in super::access::list_assets(c, org.id)? {
        let show = match audience {
            Audience::Owner => true,
            Audience::Public => super::access::asset_public(org, &asset, false),
        };
        if !show {
            continue;
        }
        let mut name = asset.name.clone();
        if asset.archived {
            name = format!("[archived] {name}");
        }
        assets.push(AssetSlot { name, download: format!("downloads/{}", asset.id) });
    }
    Ok(Page {
        org_name: org.name.clone(),
        org_description: org.description.clone(),
        brand_title: brand_title.to_string(),
        brand_motto: brand_motto.to_string(),
        theme_css: theme_css.to_string(),
        posts,
        assets,
    })
}

/// Render the organization's template for `audience`.
pub fn render_org(c: &Connection, slug: &str, brand_title: &str, brand_motto: &str, theme_css: &str, audience: Audience) -> Result<String> {
    let org = get_org(c, slug)?.ok_or_else(|| anyhow!("no organization {slug:?}"))?;
    let template = source(c, org.id)?;
    let page = page_of(c, &org, brand_title, brand_motto, theme_css, audience)?;
    render(&template, &page)
}

fn check_template(html: &str) -> Result<String> {
    if html.len() > 64 * 1024 {
        bail!("a template is at most 64 KiB");
    }
    if html.contains('\0') {
        bail!("a template cannot contain a null");
    }
    Ok(html.to_string())
}

fn expand<T>(template: &str, tag: &str, rows: &[T], mut one: impl FnMut(&str, &T) -> String) -> Result<String> {
    let open = format!("{{{{#{tag}}}}}");
    let close = format!("{{{{/{tag}}}}}");
    let mut out = String::new();
    let mut rest = template;
    while let Some(i) = rest.find(&open) {
        out.push_str(&rest[..i]);
        let after = &rest[i + open.len()..];
        let Some(j) = after.find(&close) else { bail!("template has {open} without {close}") };
        let body = &after[..j];
        for row in rows {
            out.push_str(&one(body, row));
        }
        rest = &after[j + close.len()..];
    }
    if rest.contains(&close) {
        bail!("template has {close} without {open}");
    }
    out.push_str(rest);
    Ok(out)
}

fn fill(body: &str, pairs: &[(&str, &str)]) -> String {
    let mut s = body.to_string();
    for (key, value) in pairs {
        s = s.replace(&format!("{{{{{key}}}}}"), &escape(value));
    }
    s
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Paint from the theme that is on screen.
pub fn paint_of(theme: &crate::tui::theme::Theme) -> ThemePaint {
    fn hex(c: ratatui::style::Color) -> String {
        let [r, g, b] = crate::tui::theme::rgb_bytes(c);
        format!("#{r:02x}{g:02x}{b:02x}")
    }
    ThemePaint { bg: hex(theme.bg), fg: hex(theme.fg), accent: hex(theme.accent), muted: hex(theme.muted) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brands::{builtin_default, builtin_plain};
    use crate::branding::SLOGAN;
    use crate::orgs::Visibility;

    fn conn() -> Connection {
        let d = std::env::temp_dir().join(format!(
            "acp-tpl-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&d).unwrap();
        super::super::org_conn(&d.join("db.sqlite")).unwrap()
    }

    fn person() -> String {
        let secret = acp_red_star_node_core::node::NodeSecret::generate();
        data_encoding::HEXLOWER.encode(&secret.sign_pub)
    }

    #[test]
    fn the_default_names_the_fonts_and_takes_the_title_from_the_brand() {
        assert!(DEFAULT_TEMPLATE.contains("Cinzel"));
        assert!(DEFAULT_TEMPLATE.contains("Lora"));
        assert!(DEFAULT_TEMPLATE.contains("serif"));
        assert!(!DEFAULT_TEMPLATE.contains(SLOGAN));
        assert!(!DEFAULT_TEMPLATE.contains("ACP Red Star"));
        assert!(!std::path::Path::new("assets/Cinzel.ttf").exists());
        let plain = builtin_plain();
        let page = Page {
            org_name: "Hold".into(),
            org_description: "local".into(),
            brand_title: plain.title.clone(),
            brand_motto: plain.motto.clone(),
            theme_css: ThemePaint::neutral().css(),
            posts: vec![],
            assets: vec![],
        };
        let html = render(DEFAULT_TEMPLATE, &page).unwrap();
        assert!(html.contains("Red Star"));
        assert!(!html.contains(SLOGAN), "{html}");
        assert!(html.contains("Cinzel") && html.contains("Lora"));
        let branded = builtin_default();
        let page = Page { brand_title: branded.title, brand_motto: branded.motto, ..page };
        let html = render(DEFAULT_TEMPLATE, &page).unwrap();
        assert!(html.contains(SLOGAN));
        assert!(html.contains("Red Star"));
    }

    #[test]
    fn a_template_edit_is_in_the_rendered_text_and_private_rows_stay_out() {
        let c = conn();
        let owner = person();
        super::super::create_org(&c, "hold", "Rebel Hold", "secret-description", Visibility::Private, Visibility::Private, &owner).unwrap();
        super::super::add_post(&c, "hold", "Secret", "hidden-body", None).unwrap();
        let edited = "<h1>{{brand_title}}</h1><p>marker-line</p>{{#posts}}<article>{{title}} {{body}}</article>{{/posts}}";
        let org = get_org(&c, "hold").unwrap().unwrap();
        set_source(&c, org.id, edited).unwrap();
        let public = render_org(&c, "hold", "Red Star", "", &ThemePaint::neutral().css(), Audience::Public).unwrap();
        assert!(public.contains("marker-line"));
        assert!(public.contains("Red Star"));
        assert!(!public.contains("hidden-body"));
        assert!(!public.contains("Secret"));
        let owner_html = render_org(&c, "hold", "Red Star", "", &ThemePaint::neutral().css(), Audience::Owner).unwrap();
        assert!(owner_html.contains("hidden-body"));
        reset_source(&c, org.id).unwrap();
        let again = source(&c, org.id).unwrap();
        assert!(again.contains("Cinzel"));
        assert!(!again.contains("marker-line"));
    }

    #[test]
    fn a_broken_loop_is_refused() {
        let page = Page {
            org_name: "H".into(),
            org_description: String::new(),
            brand_title: "Red Star".into(),
            brand_motto: String::new(),
            theme_css: String::new(),
            posts: vec![],
            assets: vec![],
        };
        assert!(render("{{#posts}}<p>{{title}}</p>", &page).is_err());
    }
}
