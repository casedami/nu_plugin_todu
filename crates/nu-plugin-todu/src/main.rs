//! Entry point for the `nu_plugin_todu` binary

use nu_plugin::{serve_plugin, MsgPackSerializer};
use todu_plugin::ToduPlugin;

fn main() {
    serve_plugin(&ToduPlugin::lazy(), MsgPackSerializer)
}
