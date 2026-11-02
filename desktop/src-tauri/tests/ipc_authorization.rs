use tauri::test::{mock_builder, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{ipc::InvokeBody, Manager, WebviewUrl, WebviewWindowBuilder};

/// Stand-in with the same name and guard as the real command, which is bound to the real window type.
#[tauri::command]
fn get_runtime_info<R: tauri::Runtime>(webview: tauri::Webview<R>) -> Result<serde_json::Value, pilot_desktop_lib::error::AppError> {
    pilot_desktop_lib::commands::authorize_webview(&webview)?;
    Ok(serde_json::json!({ "architecture": std::env::consts::ARCH }))
}

fn app() -> tauri::App<MockRuntime> {
    mock_builder()
        .invoke_handler(tauri::generate_handler![get_runtime_info])
        .build(tauri::generate_context!())
        .expect("mock app")
}

fn request(url: &str) -> InvokeRequest {
    InvokeRequest {
        cmd: "get_runtime_info".into(),
        callback: tauri::ipc::CallbackFn(0),
        error: tauri::ipc::CallbackFn(1),
        url: url.parse().unwrap(),
        body: InvokeBody::default(),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    }
}

fn call(app: &tauri::App<MockRuntime>, label: &str, origin: &str) -> Result<serde_json::Value, serde_json::Value> {
    let webview = app.get_webview_window(label).expect("window");
    tauri::test::get_ipc_response(&webview, request(origin)).map(|body| body.deserialize().unwrap())
}

#[test]
fn only_the_launcher_window_may_invoke_native_commands() {
    let app = app();
    WebviewWindowBuilder::new(&app, "main", WebviewUrl::App("index.html".into())).build().unwrap();
    let dashboard = WebviewUrl::External("http://127.0.0.1:7000/".parse().unwrap());
    WebviewWindowBuilder::new(&app, "dashboard-main-00000001", dashboard).build().unwrap();
    WebviewWindowBuilder::new(&app, "popup", WebviewUrl::App("index.html".into())).build().unwrap();

    let allowed = call(&app, "main", "tauri://localhost/").expect("launcher is allowed");
    assert_eq!(allowed["architecture"], std::env::consts::ARCH);

    for (label, origin) in [
        ("dashboard-main-00000001", "http://127.0.0.1:7000/"),
        ("popup", "tauri://localhost/"),
        ("main", "http://127.0.0.1:7000/"),
        ("main", "https://example.com/"),
    ] {
        let error = call(&app, label, origin).expect_err(&format!("{label} at {origin} must be refused"));
        let text = error.to_string().to_lowercase();
        assert!(text.contains("not allowed") || text.contains("unauthorized") || text.contains("may not use"), "{label} {origin}: {text}");
    }
}
