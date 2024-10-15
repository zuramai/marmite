use chrono::{NaiveDate, NaiveDateTime};
use comrak::{markdown_to_html, ComrakOptions};
use frontmatter_gen::{extract, Frontmatter, Value};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use tera::{Context, Tera};
use walkdir::WalkDir;

fn main() {
    // Argument Parsing
    let args: Vec<String> = std::env::args().collect();
    let folder = PathBuf::from(&args[1]);

    // Initialize site data
    let marmite = fs::read_to_string("marmite.yaml").expect("Unable to read marmite.yaml");
    let site: Site = serde_yaml::from_str(&marmite).expect("Failed to parse YAML");
    let mut site_data = SiteData::new(&site);

    // Walk through the content directory
    for entry in WalkDir::new(folder.join(site_data.site.content_path)) {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_file() && path.extension().unwrap() == "md" {
            process_file(path, &mut site_data);
        }
    }

    // Sort posts by date (newest first)
    site_data.posts.sort_by(|a, b| b.date.cmp(&a.date));
    // Sort pages on title
    site_data.pages.sort_by(|a, b| b.title.cmp(&a.title));

    // Create the output directory
    let output_dir = folder.join(site_data.site.site_path);
    fs::create_dir_all(&output_dir).expect("Unable to create output directory");

    // Initialize Tera templates
    let tera = match Tera::new(format!("{}/**/*", site_data.site.templates_path).as_str()) {
        Ok(t) => t,
        Err(e) => {
            println!("Parsing error(s): {}", e);
            std::process::exit(1);
        }
    };
    // Render templates
    render_templates(&site_data, &tera, &output_dir);

    // TODO: Move static and media folders to the site.

    println!("Site generated at: {}/", site_data.site.site_path);
}

#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
struct Content {
    title: String,
    slug: String,
    html: String,
    tags: Vec<String>,
    date: Option<NaiveDateTime>,
    show_in_menu: bool,
}

struct SiteData<'a> {
    site: &'a Site<'a>,
    posts: Vec<Content>,
    pages: Vec<Content>,
}

impl<'a> SiteData<'a> {
    fn new(site: &'a Site) -> Self {
        SiteData {
            site,
            posts: Vec::new(),
            pages: Vec::new(),
        }
    }
}

fn parse_front_matter(content: &str) -> (Frontmatter, &str) {
    if content.starts_with("---") {
        let (frontmatter, markdown) = extract(&content).unwrap();
        return (frontmatter, markdown);
    } else {
        let frontmatter = Frontmatter::new();
        return (frontmatter, content);
    }
}

fn process_file(path: &Path, site_data: &mut SiteData) {
    let file_content = fs::read_to_string(path).expect("Failed to read file");
    let (frontmatter, markdown) = parse_front_matter(&file_content);
    // TODO: Trim empty first and trailing lines of markdown
    let html = markdown_to_html(markdown, &ComrakOptions::default());

    let title = get_title(&frontmatter, markdown).clone();
    let tags = get_tags(&frontmatter);
    let slug = get_slug(&frontmatter, &path);
    let date = get_date(&frontmatter, &path);
    let show_in_menu = get_show_in_menu(&frontmatter);

    let content = Content {
        title,
        slug,
        tags,
        html,
        date,
        show_in_menu,
    };

    if date.is_some() {
        site_data.posts.push(content);
    } else {
        site_data.pages.push(content);
    }
}

fn get_show_in_menu(frontmatter: &Frontmatter) -> bool {
    if let Some(show_in_menu) = frontmatter.get("show_in_menu") {
        return show_in_menu.as_bool().unwrap();
    }
    false
}

fn get_date(frontmatter: &Frontmatter, path: &Path) -> Option<NaiveDateTime> {
    if let Some(input) = frontmatter.get("date") {
        if let Ok(date) =
            NaiveDateTime::parse_from_str(&input.as_str().unwrap(), "%Y-%m-%d %H:%M:%S")
        {
            return Some(date);
        } else if let Ok(date) =
            NaiveDateTime::parse_from_str(&input.as_str().unwrap(), "%Y-%m-%d %H:%M")
        {
            return Some(date);
        } else if let Ok(date) = NaiveDate::parse_from_str(&input.as_str().unwrap(), "%Y-%m-%d") {
            // Add a default time (00:00:00)
            return date.and_hms_opt(0, 0, 0);
        } else {
            println!(
                "ERROR: Invalid date format {} when parsing {}",
                input.to_string_representation(),
                path.display()
            );
            process::exit(1);
        }
    }
    None
}

fn get_slug<'a>(frontmatter: &'a Frontmatter, path: &'a Path) -> String {
    match frontmatter.get("slug") {
        Some(Value::String(slug)) => slug.to_string(),
        _ => path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap()
            .to_string(),
    }
}

fn get_title<'a>(frontmatter: &'a Frontmatter, html: &'a str) -> String {
    match frontmatter.get("title") {
        Some(Value::String(t)) => t.to_string(),
        _ => html
            .lines()
            .next()
            .unwrap_or("")
            .trim_start_matches("#")
            .trim()
            .to_string(),
    }
}

fn get_tags(frontmatter: &Frontmatter) -> Vec<String> {
    let tags: Vec<String> = match frontmatter.get("tags") {
        Some(Value::Array(tags)) => tags
            .iter()
            .map(Value::to_string)
            .map(|t| t.trim_matches('"').to_string())
            .collect(),
        Some(Value::String(tags)) => tags
            .split(",")
            .map(|t| t.trim())
            .map(String::from)
            .collect(),
        _ => Vec::new(),
    };
    tags
}

fn render_templates(site_data: &SiteData, tera: &Tera, output_dir: &Path) {
    // Render index.html
    let mut context = Context::new();
    context.insert("site", &site_data.site);
    context.insert("pages", &site_data.pages);
    context.insert("posts", &site_data.posts);
    context.insert("title", "Blog Posts"); // Get from marmite.yaml
    let index_output = tera.render("list.html", &context).unwrap();
    fs::write(output_dir.join("index.html"), index_output).expect("Unable to write file");

    // // Render individual posts and pages
    for post in &site_data.posts {
        let mut post_context = Context::new();
        post_context.insert("site", &site_data.site);
        post_context.insert("pages", &site_data.pages);
        post_context.insert("title", &post.title);
        post_context.insert("content", &post);
        let post_output = tera.render("content.html", &post_context).unwrap();
        fs::write(output_dir.join(format!("{}.html", post.slug)), post_output)
            .expect("Unable to write post");
    }

    for page in &site_data.pages {
        let mut page_context = Context::new();
        page_context.insert("site", &site_data.site);
        page_context.insert("pages", &site_data.pages);
        page_context.insert("title", &page.title);
        page_context.insert("content", &page);
        let page_output = tera.render("content.html", &page_context).unwrap();
        fs::write(output_dir.join(format!("{}.html", page.slug)), page_output)
            .expect("Unable to write page");
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
struct Site<'a> {
    #[serde(default = "default_name")]
    name: &'a str,
    #[serde(default = "default_tagline")]
    tagline: &'a str,
    #[serde(default = "default_url")]
    url: &'a str,
    #[serde(default = "default_footer")]
    footer: &'a str,
    #[serde(default = "default_pagination")]
    pagination: u32,
    #[serde(default = "default_list_title")]
    list_title: &'a str,
    #[serde(default = "default_tags_title")]
    tags_title: &'a str,
    #[serde(default = "default_content_path")]
    content_path: &'a str,
    #[serde(default = "default_templates_path")]
    templates_path: &'a str,
    #[serde(default = "default_static_path")]
    static_path: &'a str,
    #[serde(default = "default_media_path")]
    media_path: &'a str,
    #[serde(default = "default_site_path")]
    site_path: &'a str,
}

fn default_name() -> &'static str {
    "Marmite Site"
}

fn default_tagline() -> &'static str {
    "A website generated with Marmite"
}

fn default_url() -> &'static str {
    "https://example.com"
}

fn default_footer() -> &'static str {
    r#"<a href="https://creativecommons.org/licenses/by-nc-sa/4.0/">CC-BY_NC-SA</a> | Site generated with <a href="https://github.com/rochacbruno/marmite">Marmite</a>"#
}

fn default_pagination() -> u32 {
    10
}

fn default_list_title() -> &'static str {
    "Posts"
}

fn default_tags_title() -> &'static str {
    "Tags"
}

fn default_site_path() -> &'static str {
    "site"
}

fn default_content_path() -> &'static str {
    "content"
}

fn default_templates_path() -> &'static str {
    "templates"
}

fn default_static_path() -> &'static str {
    "static"
}

fn default_media_path() -> &'static str {
    "content/media"
}
