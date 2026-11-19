//! The Activity returns a persisted tree grant. Attach each catalog or editor
//! thread to the VM for the duration of its ContentResolver call.
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
fn activity<T>(
    f: impl FnOnce(&mut JNIEnv<'_>, &JObject<'_>) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    let context = ndk_context::android_context();
    // android-activity owns both the VM and global Activity reference until shutdown.
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) }?;
    let mut env = vm.attach_current_thread()?;
    env.with_local_frame(16, |env| {
        let object = unsafe { JObject::from_raw(context.context().cast()) };
        let result = f(env, &object);
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
    if let Some(tx) = PICKER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
    {
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
