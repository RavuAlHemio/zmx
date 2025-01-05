fn main() {
    embed_resource::compile("winzmx.rc", embed_resource::NONE)
        .manifest_optional().unwrap()
}
