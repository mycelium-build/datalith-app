//! The Activity returns a persisted tree grant. Attach each catalog or editor
//! thread to the VM for the duration of its `ContentResolver` call.
use datalith_vault::source::provider::{DocumentProvider, Mount};
use futures_channel::oneshot;
use jni::{
    JNIEnv, JavaVM,
    objects::{JByteArray, JClass, JObject, JString, JValue},
};
use std::{
    io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

type PickResult = anyhow::Result<Option<String>>;
static PICKER: Mutex<Option<oneshot::Sender<PickResult>>> = Mutex::new(None);
/// The running `DatalithActivity`, registered from its `onCreate`.
static ACTIVITY: Mutex<Option<jni::objects::GlobalRef>> = Mutex::new(None);

#[unsafe(no_mangle)]
extern "system" fn Java_build_mycelium_datalith_DatalithActivity_attach(
    env: JNIEnv<'_>,
    _class: JClass<'_>,
    activity: JObject<'_>,
) {
    if let Ok(reference) = env.new_global_ref(activity) {
        *ACTIVITY
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(reference);
    }
}

/// Runs `f` with the Activity, which hosts dialogs, pickers and system bars.
/// The JNI context GPUI exposes is the Application, which has none of them.
fn activity<T>(
    f: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let context = ndk_context::android_context();
    // android-activity owns both the VM and global Activity reference until shutdown.
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) }?;
    let mut env = vm.attach_current_thread()?;
    let registered = ACTIVITY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    env.with_local_frame(16, |env| {
        let fallback = unsafe { JObject::from_raw(context.context().cast()) };
        let object: &JObject<'_> = registered.as_ref().map_or(&fallback, |r| r.as_obj());
        let result = f(env, object);
        if env.exception_check()? {
            let error = env.exception_occurred()?;
            env.exception_clear()?;
            let string = JString::from(
                env.call_method(error, "toString", "()Ljava/lang/String;", &[])?
                    .l()?,
            );
            anyhow::bail!("{}", env.get_string(&string)?.to_string_lossy());
        }
        result
    })
}
pub fn pick() -> anyhow::Result<oneshot::Receiver<PickResult>> {
    let (tx, rx) = oneshot::channel();
    {
        let mut picker = PICKER
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        anyhow::ensure!(picker.is_none(), "Folder picker already open");
        *picker = Some(tx);
    }
    if let Err(error) = activity(|env, app| {
        env.call_method(app, "chooseVault", "()V", &[])?;
        Ok(())
    }) {
        PICKER
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        return Err(error);
    }
    Ok(rx)
}
#[unsafe(no_mangle)]
extern "system" fn Java_build_mycelium_datalith_DatalithActivity_folderPicked(
    mut env: JNIEnv<'_>,
    _: JClass<'_>,
    uri: JString<'_>,
    error: JString<'_>,
) {
    let result = (|| -> PickResult {
        if !error.is_null() {
            anyhow::bail!("{}", env.get_string(&error)?.to_string_lossy());
        }
        Ok(if uri.is_null() {
            None
        } else {
            Some(env.get_string(&uri)?.into())
        })
    })();
    let waiting = PICKER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    if let Some(tx) = waiting {
        let _ = tx.send(result);
    }
}
struct Provider(String);
impl Provider {
    fn operation(
        &self,
        op: &str,
        path: &Path,
        to: Option<&Path>,
        expected: Option<&[u8]>,
        content: Option<&[u8]>,
    ) -> io::Result<Vec<u8>> {
        activity(|env, app| {
            let grant = env.new_string(&self.0)?;
            let operation = env.new_string(op)?;
            let path = env.new_string(
                path.to_str()
                    .ok_or_else(|| io::Error::other("Invalid document name"))?,
            )?;
            let destination = env.new_string(to.and_then(Path::to_str).unwrap_or(""))?;
            let expected = match expected {
                Some(bytes) => JObject::from(env.byte_array_from_slice(bytes)?),
                None => JObject::null(),
            };
            let content = match content {
                Some(bytes) => JObject::from(env.byte_array_from_slice(bytes)?),
                None => JObject::null(),
            };
            let arguments = [
                &*grant,
                &*operation,
                &*path,
                &*destination,
                &expected,
                &content,
            ]
            .map(JValue::Object);
            let result = env.call_method(app, "vaultOperation",
                "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;[B[B)[B",
                &arguments)?.l()?;
            Ok(env.convert_byte_array(JByteArray::from(result))?)
        })
        .map_err(io::Error::other)
    }
}
impl DocumentProvider for Provider {
    fn attributes(&self, p: &Path) -> io::Result<datalith_vault::source::provider::Attributes> {
        let [size, modified_ms]: [i64; 2] =
            serde_json::from_slice(&self.operation("attributes", p, None, None, None)?)
                .map_err(io::Error::other)?;
        Ok(datalith_vault::source::provider::Attributes {
            size,
            modified_ns: modified_ms.saturating_mul(1_000_000),
            created_ns: 0,
        })
    }
    fn list(&self, p: &Path) -> io::Result<Vec<String>> {
        serde_json::from_slice(&self.operation("list", p, None, None, None)?)
            .map_err(io::Error::other)
    }
    fn is_directory(&self, p: &Path) -> io::Result<bool> {
        Ok(self.operation("directory", p, None, None, None)? == [1])
    }
    fn read(&self, p: &Path) -> io::Result<Vec<u8>> {
        self.operation("read", p, None, None, None)
    }
    fn save(&self, p: &Path, base: Option<&[u8]>, data: &[u8]) -> io::Result<()> {
        self.operation("save", p, None, base, Some(data)).map(drop)
    }
    fn mkdir(&self, p: &Path) -> io::Result<()> {
        self.operation("mkdir", p, None, None, None).map(drop)
    }
    fn rename(&self, a: &Path, b: &Path) -> io::Result<()> {
        self.operation("rename", a, Some(b), None, None).map(drop)
    }
}
pub struct Folder {
    pub path: PathBuf,
    pub label: String,
    pub bookmark: String,
    _mount: Mount,
}
impl Folder {
    pub fn resolve(grant: &str) -> anyhow::Result<Self> {
        let provider = Arc::new(Provider(grant.into()));
        anyhow::ensure!(
            provider.is_directory(Path::new(""))?,
            "The granted folder is unavailable"
        );
        let label =
            String::from_utf8(provider.operation("name", Path::new(""), None, None, None)?)?;
        let mount = Mount::new(
            &datalith_vault::draft::storage_key(grant.as_bytes()),
            provider,
        )?;
        Ok(Self {
            path: mount.root().to_owned(),
            bookmark: grant.into(),
            _mount: mount,
            label,
        })
    }
}

pub fn release(grant: &str) -> anyhow::Result<()> {
    activity(|env, app| {
        let grant = env.new_string(grant)?;
        env.call_method(
            app,
            "forgetVault",
            "(Ljava/lang/String;)V",
            &[JValue::Object(&grant)],
        )?;
        Ok(())
    })
}

/// Gives the status and navigation bars the theme background, with dark icons
/// on a light theme so the clock and battery stay readable.
///
/// Called on every render: it does nothing until the colors change, and keeps
/// trying while the Activity is not yet the JNI context during startup.
pub fn set_system_bars(background: gpui_kit::Hsla, light: bool) {
    static APPLIED: Mutex<Option<(u32, bool)>> = Mutex::new(None);
    let rgba: u32 = gpui_kit::Rgba::from(background).into();
    let mut applied = APPLIED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if *applied == Some((rgba, light)) {
        return;
    }
    // Android colors are ARGB.
    let argb = i32::from_be_bytes(rgba.rotate_right(8).to_be_bytes());
    let result = activity(|env, app| {
        env.call_method(
            app,
            "setSystemBars",
            "(IZ)V",
            &[JValue::Int(argb), JValue::Bool(u8::from(light))],
        )?;
        Ok(())
    });
    if result.is_ok() {
        *applied = Some((rgba, light));
    }
}

/// The body text size for Android's font size setting, in points.
pub fn preferred_text_size() -> anyhow::Result<f32> {
    activity(|env, app| Ok(env.call_method(app, "preferredTextSize", "()F", &[])?.f()?))
}

/// True once: the first time the Activity reports the system text size.
pub fn text_size_first_available() -> bool {
    use std::sync::atomic::{AtomicBool, Ordering};
    static SIZED: AtomicBool = AtomicBool::new(false);
    !SIZED.load(Ordering::Relaxed)
        && preferred_text_size().is_ok()
        && !SIZED.swap(true, Ordering::Relaxed)
}

pub fn show_options(title: &str, items: &[String], selected: i32) -> anyhow::Result<()> {
    activity(|env, app| {
        let title = env.new_string(title)?;
        let array = env.new_object_array(
            i32::try_from(items.len())?,
            "java/lang/String",
            JObject::null(),
        )?;
        for (index, item) in items.iter().enumerate() {
            let value = env.new_string(item)?;
            env.set_object_array_element(&array, i32::try_from(index)?, &value)?;
            env.delete_local_ref(value)?;
        }
        env.call_method(
            app,
            "chooseOption",
            "(Ljava/lang/String;[Ljava/lang/String;I)V",
            &[
                JValue::Object(&title),
                JValue::Object(&array),
                JValue::Int(selected),
            ],
        )?;
        Ok(())
    })
}
/// Lists commands with `AlertDialog.setItems`: plain rows, no radio buttons.
/// The destructive command is marked so the host can tint it.
pub fn show_actions(title: &str, items: &[crate::workspace::Item]) -> anyhow::Result<()> {
    activity(|env, app| {
        let title = env.new_string(title)?;
        let array = env.new_object_array(
            i32::try_from(items.len())?,
            "java/lang/String",
            JObject::null(),
        )?;
        let mut destructive = -1;
        for (index, item) in items.iter().enumerate() {
            let value = env.new_string(item.label)?;
            env.set_object_array_element(&array, i32::try_from(index)?, &value)?;
            env.delete_local_ref(value)?;
            if item.destructive {
                destructive = i32::try_from(index)?;
            }
        }
        env.call_method(
            app,
            "chooseActions",
            "(Ljava/lang/String;[Ljava/lang/String;I)V",
            &[
                JValue::Object(&title),
                JValue::Object(&array),
                JValue::Int(destructive),
            ],
        )?;
        Ok(())
    })
}
#[unsafe(no_mangle)]
extern "system" fn Java_build_mycelium_datalith_DatalithActivity_openLink(
    mut env: JNIEnv<'_>,
    _: JClass<'_>,
    url: JString<'_>,
) {
    if let Ok(url) = env.get_string(&url) {
        crate::commands::link(url.into());
    }
}
#[unsafe(no_mangle)]
extern "system" fn Java_build_mycelium_datalith_DatalithActivity_choseOption(
    _: JNIEnv<'_>,
    _: JClass<'_>,
    index: i32,
) {
    crate::commands::choice(index);
}
