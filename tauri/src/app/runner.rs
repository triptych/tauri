#[allow(unused_imports)]
use std::{
  env,
  fs::{self, read_to_string},
  path::Path,
  process::Stdio,
  thread::spawn,
};

use web_view::{builder, Content, WebView};

use super::App;
#[cfg(embedded_server)]
use crate::api::tcp::{get_available_port, port_is_available};
use tauri_api::config::get;

/// Main entry point for running the Webview
pub(crate) fn run(application: &mut App) -> crate::Result<()> {
  // setup the content using the config struct depending on the compile target
  let main_content = setup_content()?;

  // setup the server url for the embedded-server
  #[cfg(embedded_server)]
  let server_url = {
    if let Content::Url(ref url) = &main_content {
      String::from(url)
    } else {
      String::from("")
    }
  };

  // build the webview
  let webview = build_webview(
    application,
    main_content,
    if application.splashscreen_html().is_some() {
      Some(Content::Html(
        application
          .splashscreen_html()
          .expect("failed to get splashscreen_html")
          .to_string(),
      ))
    } else {
      None
    },
  )?;

  // spawn the embedded server on our server url
  #[cfg(embedded_server)]
  spawn_server(server_url)?;

  // spin up the updater process
  #[cfg(feature = "updater")]
  spawn_updater()?;

  // run the webview
  webview.run()?;

  Ok(())
}

// setup content for dev-server
#[cfg(dev)]
fn setup_content() -> crate::Result<Content<String>> {
  let config = get()?;
  if config.build.dev_path.starts_with("http") {
    #[cfg(windows)]
    {
      let exempt_output = std::process::Command::new("CheckNetIsolation")
        .args(&vec!["LoopbackExempt", "-s"])
        .output()
        .expect("failed to read LoopbackExempt -s");

      if !exempt_output.status.success() {
        panic!("Failed to execute CheckNetIsolation LoopbackExempt -s");
      }

      let output_str = String::from_utf8_lossy(&exempt_output.stdout).to_lowercase();
      if !output_str.contains("win32webviewhost_cw5n1h2txyewy") {
        println!("Running Loopback command");
        runas::Command::new("powershell")
          .args(&vec![
            "CheckNetIsolation LoopbackExempt -a -n=\"Microsoft.Win32WebViewHost_cw5n1h2txyewy\"",
          ])
          .force_prompt(true)
          .status()
          .expect("failed to run Loopback command");
      }
    }
    Ok(Content::Url(config.build.dev_path.clone()))
  } else {
    let dev_dir = &config.build.dev_path;
    let dev_path = Path::new(dev_dir).join("index.tauri.html");
    if !dev_path.exists() {
      panic!(
        "Couldn't find 'index.tauri.html' inside {}; did you forget to run 'tauri dev'?",
        dev_dir
      );
    }
    Ok(Content::Html(read_to_string(dev_path)?))
  }
}

// setup content for embedded server
#[cfg(embedded_server)]
fn setup_content() -> crate::Result<Content<String>> {
  let (port, valid) = setup_port()?;
  let url = (if valid {
    setup_server_url(port)
  } else {
    Err(anyhow::anyhow!("invalid port"))
  })
  .expect("Unable to setup URL");

  Ok(Content::Url(url))
}

// setup content for no-server
#[cfg(no_server)]
fn setup_content() -> crate::Result<Content<String>> {
  let html = include_str!(concat!(env!("OUT_DIR"), "/index.tauri.html"));
  Ok(Content::Html(html.to_string()))
}

// get the port for the embedded server
#[cfg(embedded_server)]
fn setup_port() -> crate::Result<(String, bool)> {
  let config = get()?;
  match config.tauri.embedded_server.port {
    tauri_api::config::Port::Random => match get_available_port() {
      Some(available_port) => Ok((available_port.to_string(), true)),
      None => Ok(("0".to_string(), false)),
    },
    tauri_api::config::Port::Value(port) => {
      let port_valid = port_is_available(port);
      Ok((port.to_string(), port_valid))
    }
  }
}

// setup the server url for embedded server
#[cfg(embedded_server)]
fn setup_server_url(port: String) -> crate::Result<String> {
  let config = get()?;
  let mut url = format!("{}:{}", config.tauri.embedded_server.host, port);
  if !url.starts_with("http") {
    url = format!("http://{}", url);
  }
  Ok(url)
}

// spawn the embedded server
#[cfg(embedded_server)]
fn spawn_server(server_url: String) -> crate::Result<()> {
  spawn(move || {
    let server = tiny_http::Server::http(server_url.replace("http://", "").replace("https://", ""))
      .expect("Unable to spawn server");
    for request in server.incoming_requests() {
      let url = match request.url() {
        "/" => "/index.tauri.html",
        url => url,
      }
      .to_string();
      request
        .respond(crate::server::asset_response(&url))
        .expect("unable to setup response");
    }
  });

  Ok(())
}

// spawn an updater process.
#[cfg(feature = "updater")]
fn spawn_updater() -> crate::Result<()> {
  spawn(|| {
    tauri_api::command::spawn_relative_command("updater".to_string(), Vec::new(), Stdio::inherit())
      .expect("Unable to spawn relative command");
  });
  Ok(())
}

// build the webview struct
fn build_webview(
  application: &mut App,
  content: Content<String>,
  splashscreen_content: Option<Content<String>>,
) -> crate::Result<WebView<'_, ()>> {
  let config = get()?;
  let content_clone = match content {
    Content::Html(ref html) => Content::Html(html.clone()),
    Content::Url(ref url) => Content::Url(url.clone()),
  };
  let debug = cfg!(debug_assertions);
  // get properties from config struct
  let width = config.tauri.window.width;
  let height = config.tauri.window.height;
  let resizable = config.tauri.window.resizable;
  let fullscreen = config.tauri.window.fullscreen;
  let title = config.tauri.window.title.clone().into_boxed_str();

  let has_splashscreen = splashscreen_content.is_some();
  let mut initialized_splashscreen = false;

  let mut webview = builder()
    .title(Box::leak(title))
    .size(width, height)
    .resizable(resizable)
    .debug(debug)
    .user_data(())
    .invoke_handler(move |webview, arg| {
      if arg == r#"{"cmd":"__initialized"}"# {
        let source = if has_splashscreen && !initialized_splashscreen {
          initialized_splashscreen = true;
          "splashscreen"
        } else {
          "window-1"
        };
        application.run_setup(webview, source.to_string());
      } else if arg == r#"{"cmd":"closeSplashscreen"}"# {
        let content_href = match content_clone {
          Content::Html(ref html) => html,
          Content::Url(ref url) => url,
        };
        webview.eval(&format!(r#"window.location.href = "{}""#, content_href))?;
      } else {
        let handler_error;
        if let Err(tauri_handle_error) = crate::endpoints::handle(webview, arg) {
          let tauri_handle_error_str = tauri_handle_error.to_string();
          if tauri_handle_error_str.contains("unknown variant") {
            let handled_by_app = application.run_invoke_handler(webview, arg);
            handler_error = if let Err(e) = handled_by_app {
              Some(e.replace("'", "\\'"))
            } else {
              let handled = handled_by_app.expect("failed to check if the invoke was handled");
              if handled {
                None
              } else {
                Some(tauri_handle_error_str)
              }
            };
          } else {
            handler_error = Some(tauri_handle_error_str);
          }

          if let Some(handler_error_message) = handler_error {
            webview.eval(&get_api_error_message(arg, handler_error_message))?;
          }
        }
      }

      Ok(())
    })
    .content(if splashscreen_content.is_some() {
      splashscreen_content.expect("failed to get splashscreen content")
    } else {
      content
    })
    .build()?;

  webview.set_fullscreen(fullscreen);

  if has_splashscreen {
    let env_var = envmnt::get_or("TAURI_DIR", "../dist");
    let path = Path::new(&env_var);
    let contents = fs::read_to_string(path.join("/tauri.js"))?;
    // inject the tauri.js entry point
    webview
      .handle()
      .dispatch(move |_webview| _webview.eval(&contents))?;
  }

  Ok(webview)
}

// Formats an invoke handler error message to print to console.error
fn get_api_error_message(arg: &str, handler_error_message: String) -> String {
  format!(
    r#"console.error('failed to match a command for {}, {}')"#,
    arg.replace("'", "\\'"),
    handler_error_message
  )
}

#[cfg(test)]
mod test {
  use proptest::prelude::*;
  use std::env;
  use web_view::Content;

  #[cfg(not(feature = "embedded-server"))]
  use std::{fs::read_to_string, path::Path};

  #[test]
  fn check_setup_content() {
    let tauri_dir = match option_env!("TAURI_DIR") {
      Some(d) => d.to_string(),
      None => env::current_dir()
        .unwrap()
        .into_os_string()
        .into_string()
        .expect("Unable to convert to normal String"),
    };
    env::set_current_dir(tauri_dir).expect("failed to change cwd");
    let res = super::setup_content();

    #[cfg(embedded_server)]
    match res {
      Ok(Content::Url(u)) => assert!(u.contains("http://")),
      _ => panic!("setup content failed"),
    }

    #[cfg(no_server)]
    match res {
      Ok(Content::Html(s)) => {
        let dist_dir = match option_env!("TAURI_DIST_DIR") {
          Some(d) => d.to_string(),
          None => env::current_dir()
            .unwrap()
            .into_os_string()
            .into_string()
            .expect("Unable to convert to normal String"),
        };
        assert_eq!(
          s,
          read_to_string(Path::new(&dist_dir).join("index.tauri.html")).unwrap()
        );
      }
      _ => panic!("setup content failed"),
    }

    #[cfg(dev)]
    {
      let config = tauri_api::config::get().expect("unable to setup default config");
      match res {
        Ok(Content::Url(dp)) => assert_eq!(dp, config.build.dev_path),
        Ok(Content::Html(s)) => {
          let dev_dir = &config.build.dev_path;
          let dev_path = Path::new(dev_dir).join("index.tauri.html");
          assert_eq!(
            s,
            read_to_string(dev_path).expect("failed to read dev path")
          );
        }
        _ => panic!("setup content failed"),
      }
    }
  }

  #[cfg(embedded_server)]
  #[test]
  fn check_setup_port() {
    let res = super::setup_port();
    match res {
      Ok((_s, _b)) => {}
      _ => panic!("setup port failed"),
    }
  }

  proptest! {
    #![proptest_config(ProptestConfig::with_cases(10000))]
    #[cfg(embedded_server)]
    #[test]
    fn check_server_url(port in (any::<u32>().prop_map(|v| v.to_string()))) {
      let p = port.clone();

      let res = super::setup_server_url(port);

      match res {
        Ok(url) => assert!(url.contains(&p)),
        Err(e) => panic!("setup_server_url Err {:?}", e.to_string())
      }
    }
  }
}
