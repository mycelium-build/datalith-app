use android_activity::AndroidApp;
use gpui_mobile::android::jni as platform;

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    platform::install_panic_hook();
    let _platform = platform::init_platform(&app);
    if let (Some(platform), Some(files)) = (platform::shared_platform(), app.internal_data_path()) {
        datalith_mobile::run_android(
            gpui_kit::Application::with_platform(platform.into_rc()),
            &files,
        );
    }
}
