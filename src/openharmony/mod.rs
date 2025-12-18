use crate::{Result, WebViewAttributes, Rect, RGBA};
use std::sync::{Mutex, OnceLock};
use std::collections::HashMap;
use napi::threadsafe_function::{ThreadsafeFunction, ErrorStrategy, ThreadsafeFunctionCallMode};

pub struct InnerWebView {
    id: String,
}

static HANDLERS: OnceLock<Mutex<HashMap<String, Box<dyn Fn(crate::http::Request<String>) + Send>>>> = OnceLock::new();
static SCRIPT_EXECUTOR: OnceLock<ThreadsafeFunction<String, ErrorStrategy::Fatal>> = OnceLock::new();
static PROTOCOL_HANDLERS: OnceLock<Mutex<HashMap<String, Box<dyn Fn(crate::WebViewId, crate::http::Request<Vec<u8>>, crate::RequestAsyncResponder) + Send>>>> = OnceLock::new();

pub fn register_script_executor(tsfn: ThreadsafeFunction<String, ErrorStrategy::Fatal>) {
    if SCRIPT_EXECUTOR.set(tsfn).is_err() {
        log::warn!("Script executor already registered");
    }
}

pub fn on_ipc_message(id: &str, msg: String) {
    if let Some(handlers) = HANDLERS.get() {
        if let Some(handler) = handlers.lock().unwrap().get(id) {
             let req = crate::http::Request::builder()
                .uri(format!("ipc://{}", id))
                .body(msg)
                .unwrap();
             handler(req);
        }
    }
}

pub fn handle_request(url: String) -> Option<Vec<u8>> {
    // Basic synchronous handling for now, matching the current NAPI structure
    // We need to parse the scheme from the URL
    if let Some(protocols) = PROTOCOL_HANDLERS.get() {
        let protocols = protocols.lock().unwrap();
        // find protocol handler
        if let Some(scheme_end) = url.find("://") {
            let scheme = &url[0..scheme_end];
             if let Some(handler) = protocols.get(scheme) {
                 let req = crate::http::Request::builder()
                    .uri(url)
                    .body(Vec::new())
                    .unwrap();
                 
                 let (tx, rx) = std::sync::mpsc::channel();
                 // Construct RequestAsyncResponder using internal field (we are in the same crate)
                 let responder = crate::RequestAsyncResponder {
                     responder: Box::new(move |res| {
                         let _ = tx.send(res.body().to_vec());
                     }),
                 };
                 
                 // TODO: Use correct WebViewId. For now assuming "0".
                 handler("0", req, responder);
                 
                 return rx.recv().ok();
             }
        }
    }
    None
}

impl InnerWebView {
  pub fn new(
    _window: &impl raw_window_handle::HasWindowHandle,
    attributes: WebViewAttributes,
    _pl_attrs: super::PlatformSpecificWebViewAttributes,
  ) -> Result<Self> {
      let id = attributes.id.map(|s| s.to_string()).unwrap_or_else(|| "0".to_string());
      
      if let Some(ipc) = attributes.ipc_handler {
          HANDLERS.get_or_init(|| Mutex::new(HashMap::new()))
              .lock().unwrap().insert(id.clone(), ipc);
      }
      
      for (name, handler) in attributes.custom_protocols {
          PROTOCOL_HANDLERS.get_or_init(|| Mutex::new(HashMap::new()))
              .lock().unwrap().insert(name, handler);
      }
      
      Ok(Self { id })
  }

  pub fn new_as_child(
    window: &impl raw_window_handle::HasWindowHandle,
    attributes: WebViewAttributes,
    pl_attrs: super::PlatformSpecificWebViewAttributes,
  ) -> Result<Self> {
      Self::new(window, attributes, pl_attrs)
  }

  pub fn id(&self) -> crate::WebViewId {
    &self.id
  }

  pub fn url(&self) -> crate::Result<String> { Ok(String::new()) }
  
  pub fn eval(&self, js: &str, _callback: Option<impl Fn(String) + Send + 'static>) -> Result<()> {
      if let Some(tsfn) = SCRIPT_EXECUTOR.get() {
          tsfn.call(js.to_string(), ThreadsafeFunctionCallMode::NonBlocking);
      } else {
          log::warn!("Script executor not registered. Cannot eval JS.");
      }
      Ok(())
  }

  pub fn load_url(&self, _url: &str) -> Result<()> { Ok(()) }
  pub fn load_url_with_headers(&self, _url: &str, _headers: crate::http::HeaderMap) -> Result<()> { Ok(()) }
  pub fn load_html(&self, _html: &str) -> Result<()> { Ok(()) }
  pub fn reload(&self) -> Result<()> { Ok(()) }
  pub fn clear_all_browsing_data(&self) -> Result<()> { Ok(()) }
  pub fn bounds(&self) -> Result<Rect> { Ok(Rect::default()) }
  pub fn set_bounds(&self, _bounds: Rect) -> Result<()> { Ok(()) }
  pub fn set_visible(&self, _visible: bool) -> Result<()> { Ok(()) }
  pub fn focus(&self) -> Result<()> { Ok(()) }
  pub fn focus_parent(&self) -> Result<()> { Ok(()) }
  pub fn print(&self) -> Result<()> { Ok(()) }
  pub fn zoom(&self, _scale: f64) -> Result<()> { Ok(()) }
  pub fn set_background_color(&self, _color: RGBA) -> Result<()> { Ok(()) }
  pub fn cookies_for_url(&self, _url: &str) -> Result<Vec<crate::cookie::Cookie<'static>>> { Ok(vec![]) }
  pub fn set_cookie(&self, _cookie: &crate::cookie::Cookie<'_>) -> Result<()> { Ok(()) }
  pub fn delete_cookie(&self, _cookie: &crate::cookie::Cookie<'_>) -> Result<()> { Ok(()) }
  pub fn cookies(&self) -> Result<Vec<crate::cookie::Cookie<'static>>> { Ok(vec![]) }

  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn open_devtools(&self) {}
  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn close_devtools(&self) {}
  #[cfg(any(debug_assertions, feature = "devtools"))]
  pub fn is_devtools_open(&self) -> bool { false }
}

#[derive(Clone, Copy)]
pub struct JniHandle;
impl JniHandle {
    pub fn exec<F>(&self, _func: F) {}
}

pub fn platform_webview_version() -> Result<String> {
    Ok("ArkWeb".to_string())
}
