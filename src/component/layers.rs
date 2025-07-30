use askama::Template;
use askama_web::WebTemplate;

#[derive(Template, WebTemplate)]
#[template(path = "layers.html", escape = "none")]
pub struct Layers {}

pub async fn layers() -> Layers {
    Layers {}
}
