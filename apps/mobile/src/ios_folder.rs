//! A Files bookmark holds permission across launches. Its resolved URL stays
//! alive for the session and releases the security scope on drop.
use std::{
    ffi::{CStr, CString, c_char, c_void},
    path::PathBuf,
    ptr::NonNull,
};

unsafe extern "C" {
    fn datalith_pick_folder();
    fn datalith_resolve_folder(
        encoded: *const c_char,
        path: *mut *mut c_char,
        renewed: *mut *mut c_char,
    ) -> *mut c_void;
    fn datalith_release_folder(url: *mut c_void);
    fn datalith_free_folder_string(string: *mut c_char);
}

pub struct Scope(NonNull<c_void>);
// NSURL is immutable. File coordination and releasing the security scope are thread-safe.
unsafe impl Send for Scope {}
unsafe impl Sync for Scope {}

pub struct Folder {
    _mount: datalith_vault::source::provider::Mount,
    pub path: PathBuf,
    pub label: String,
    pub bookmark: String,
}
impl Folder {
    pub fn resolve(bookmark: &str) -> anyhow::Result<Self> {
        let encoded = CString::new(bookmark)?;
        let mut path = std::ptr::null_mut();
        let mut renewed = std::ptr::null_mut();
        // The Objective-C bridge retains the URL and returns independently allocated strings.
        let url = NonNull::new(unsafe {
            datalith_resolve_folder(encoded.as_ptr(), &raw mut path, &raw mut renewed)
        })
        .ok_or_else(|| {
            anyhow::anyhow!("Folder access expired. Choose the folder again in Files.")
        })?;
        let scope = std::sync::Arc::new(Scope(url));
        let root = PathBuf::from(take_string(path).unwrap_or_default());
        let bookmark = take_string(renewed).unwrap_or_else(|| bookmark.to_owned());
        anyhow::ensure!(root.is_absolute(), "Files returned an invalid folder");
        let label = root
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let id = datalith_vault::draft::storage_key(root.as_os_str().as_encoded_bytes());
        let provider = super::ios_provider::Provider {
            _scope: scope,
            root,
        };
        let mount =
            datalith_vault::source::provider::Mount::new(&id, std::sync::Arc::new(provider))?;
        Ok(Self {
            path: mount.root().to_owned(),
            bookmark,
            _mount: mount,
            label,
        })
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        unsafe {
            datalith_release_folder(self.0.as_ptr());
        }
    }
}
fn take_string(pointer: *mut c_char) -> Option<String> {
    let pointer = NonNull::new(pointer)?;
    let result = unsafe { CStr::from_ptr(pointer.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    unsafe {
        datalith_free_folder_string(pointer.as_ptr());
    }
    Some(result)
}
pub fn pick() {
    unsafe {
        datalith_pick_folder();
    }
}
thread_local! {static PICKED:std::cell::RefCell<Option<anyhow::Result<String>>>=const{std::cell::RefCell::new(None)};}
pub fn take() -> Option<anyhow::Result<String>> {
    PICKED.with_borrow_mut(Option::take)
}
#[unsafe(no_mangle)]
extern "C" fn datalith_folder_picked(bookmark: *const c_char, error: *const c_char) {
    let result = if bookmark.is_null() {
        let message = if error.is_null() {
            "Cannot save folder access".into()
        } else {
            unsafe { CStr::from_ptr(error) }
                .to_string_lossy()
                .into_owned()
        };
        Err(anyhow::anyhow!(message))
    } else {
        Ok(unsafe { CStr::from_ptr(bookmark) }
            .to_string_lossy()
            .into_owned())
    };
    PICKED.with_borrow_mut(|slot| *slot = Some(result));
    super::ios::dispatch(super::ios::FOLDER_PICKED);
}
