use askama::Template;

#[derive(Template)]
#[template(path = "layers.html", escape = "none")]
pub struct Layers {}

pub async fn layers() -> Layers {
    Layers {}
}
