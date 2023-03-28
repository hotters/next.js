use anyhow::{bail, Context, Error};
use js_sys::JsString;
use next_swc::{custom_before_pass, TransformOptions};
use std::sync::Arc;
use wasm_bindgen::{prelude::*, JsCast};
use wasm_bindgen_futures::future_to_promise;

use next_binding::swc::core::{
    base::{config::JsMinifyOptions, try_with_handler, Compiler},
    common::{
        comments::SingleThreadedComments, errors::ColorConfig, FileName, FilePathMapping,
        SourceMap, GLOBALS,
    },
    ecma::transforms::base::pass::noop,
};

pub mod mdx;

fn convert_err(err: Error) -> JsValue {
    format!("{:?}", err).into()
}

#[wasm_bindgen(js_name = "minifySync")]
pub fn minify_sync(s: JsString, opts: JsValue) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let c = compiler();

    let opts: JsMinifyOptions = serde_wasm_bindgen::from_value(opts)?;

    let value = try_with_handler(
        c.cm.clone(),
        next_binding::swc::core::base::HandlerOpts {
            color: ColorConfig::Never,
            skip_filename: false,
        },
        |handler| {
            GLOBALS.set(&Default::default(), || {
                let fm = c.cm.new_source_file(FileName::Anon, s.into());
                let program = c
                    .minify(fm, handler, &opts)
                    .context("failed to minify file")?;

                Ok(program)
            })
        },
    )
    .map_err(convert_err)?;

    Ok(serde_wasm_bindgen::to_value(&value)?)
}

#[wasm_bindgen(js_name = "minify")]
pub fn minify(s: JsString, opts: JsValue) -> js_sys::Promise {
    // TODO: This'll be properly scheduled once wasm have standard backed thread
    // support.
    future_to_promise(async { minify_sync(s, opts) })
}

#[wasm_bindgen(js_name = "transformSync")]
pub fn transform_sync(s: JsValue, opts: JsValue) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let c = compiler();
    let opts: TransformOptions = serde_wasm_bindgen::from_value(opts)?;

    let s = s.dyn_into::<js_sys::JsString>();
    let out = try_with_handler(
        c.cm.clone(),
        next_binding::swc::core::base::HandlerOpts {
            color: ColorConfig::Never,
            skip_filename: false,
        },
        |handler| {
            GLOBALS.set(&Default::default(), || {
                let out = match s {
                    Ok(s) => {
                        let fm = c.cm.new_source_file(
                            if opts.swc.filename.is_empty() {
                                FileName::Anon
                            } else {
                                FileName::Real(opts.swc.filename.clone().into())
                            },
                            s.into(),
                        );
                        let cm = c.cm.clone();
                        let file = fm.clone();
                        let comments = SingleThreadedComments::default();
                        c.process_js_with_custom_pass(
                            fm,
                            None,
                            handler,
                            &opts.swc,
                            comments.clone(),
                            |_| {
                                custom_before_pass(
                                    cm,
                                    file,
                                    &opts,
                                    comments.clone(),
                                    Default::default(),
                                )
                            },
                            |_| noop(),
                        )
                        .context("failed to process js file")?
                    }
                    Err(_) => bail!("No source passed to transform"),
                };

                Ok(out)
            })
        },
    )
    .map_err(convert_err)?;

    Ok(serde_wasm_bindgen::to_value(&out)?)
}

#[wasm_bindgen(js_name = "transform")]
pub fn transform(s: JsValue, opts: JsValue) -> js_sys::Promise {
    // TODO: This'll be properly scheduled once wasm have standard backed thread
    // support.
    future_to_promise(async { transform_sync(s, opts) })
}

/// Get global sourcemap
fn compiler() -> Arc<Compiler> {
    let cm = Arc::new(SourceMap::new(FilePathMapping::empty()));

    Arc::new(Compiler::new(cm))
}
