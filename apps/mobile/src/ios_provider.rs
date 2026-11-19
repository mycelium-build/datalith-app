//! Coordinate each operation on an external Files URL, including the version
//! check and replacement together in one writing accessor.
use datalith_vault::source::provider::DocumentProvider;
use std::{
    ffi::{CStr, CString, c_char, c_void},
    fs, io,
    os::unix::ffi::OsStrExt as _,
    path::{Path, PathBuf},
    sync::Arc,
};

pub struct Provider {
    pub _scope: Arc<super::ios_folder::Scope>,
    pub root: PathBuf,
}

type Access = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char);
unsafe extern "C" {
    fn datalith_coordinate(
        first: *const c_char,
        second: *const c_char,
        operation: i32,
        context: *mut c_void,
        access: Access,
    ) -> *mut c_char;
    fn datalith_free_folder_string(pointer: *mut c_char);
}
struct Operation<F, T> {
    work: Option<F>,
    result: Option<io::Result<T>>,
}
unsafe extern "C" fn perform<F, T>(context: *mut c_void, a: *const c_char, b: *const c_char)
where
    F: FnOnce(&Path, Option<&Path>) -> io::Result<T>,
{
    let operation = unsafe { &mut *context.cast::<Operation<F, T>>() };
    let path = |pointer| {
        Path::new(std::ffi::OsStr::from_bytes(
            unsafe { CStr::from_ptr(pointer) }.to_bytes(),
        ))
    };
    if let Some(work) = operation.work.take() {
        operation.result = Some(work(path(a), (!b.is_null()).then(|| path(b))));
    }
}
impl Provider {
    fn access<T, F>(
        &self,
        path: &Path,
        other: Option<&Path>,
        writing: bool,
        work: F,
    ) -> io::Result<T>
    where
        F: FnOnce(&Path, Option<&Path>) -> io::Result<T>,
    {
        let checked = |relative: &Path| -> io::Result<CString> {
            let mut full = self.root.clone();
            for part in relative.components() {
                let std::path::Component::Normal(name) = part else {
                    return Err(io::Error::other("Invalid document name"));
                };
                full.push(name);
                if full.is_symlink() {
                    return Err(io::Error::other(
                        "Linked files outside a vault are unsupported",
                    ));
                }
            }
            CString::new(full.as_os_str().as_bytes()).map_err(io::Error::other)
        };
        let a = checked(path)?;
        let b = other.map(checked).transpose()?;
        let mut op = Operation {
            work: Some(work),
            result: None,
        };
        let error = unsafe {
            datalith_coordinate(
                a.as_ptr(),
                b.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
                i32::from(writing),
                (&raw mut op).cast(),
                perform::<F, T>,
            )
        };
        if !error.is_null() {
            let message = unsafe { CStr::from_ptr(error) }
                .to_string_lossy()
                .into_owned();
            unsafe { datalith_free_folder_string(error) };
            return Err(io::Error::other(message));
        }
        op.result
            .unwrap_or_else(|| Err(io::Error::other("Could not coordinate access")))
    }
}
impl DocumentProvider for Provider {
    fn attributes(&self, path: &Path) -> io::Result<datalith_vault::source::provider::Attributes> {
        self.access(path, None, false, |p, _| Ok((&fs::metadata(p)?).into()))
    }
    fn list(&self, path: &Path) -> io::Result<Vec<String>> {
        self.access(path, None, false, |p, _| {
            fs::read_dir(p)?
                .filter_map(|e| match e {
                    Ok(e) if e.path().is_symlink() => None,
                    other => Some(other.and_then(|e| {
                        e.file_name()
                            .into_string()
                            .map_err(|_| io::Error::other("Invalid filename"))
                    })),
                })
                .collect()
        })
    }
    fn is_directory(&self, path: &Path) -> io::Result<bool> {
        self.access(path, None, false, |p, _| Ok(fs::metadata(p)?.is_dir()))
    }
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.access(path, None, false, |p, _| fs::read(p))
    }
    fn mkdir(&self, path: &Path) -> io::Result<()> {
        self.access(path, None, true, |p, _| fs::create_dir(p))
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.access(from, Some(to), true, |p, t| {
            let t = t.ok_or_else(|| io::Error::other("Missing destination"))?;
            if t.exists() {
                return Err(io::Error::from(io::ErrorKind::AlreadyExists));
            }
            fs::rename(p, t)
        })
    }
    fn save(&self, path: &Path, expected: Option<&[u8]>, content: &[u8]) -> io::Result<()> {
        self.access(path, None, true, |p, _| {
            use std::io::Write as _;
            if let Some(base) = expected
                && fs::read(p)? != base
            {
                return Err(io::Error::other("Document changed in another app"));
            }
            let mut tmp = tempfile::NamedTempFile::new_in(
                p.parent()
                    .ok_or_else(|| io::Error::other("Missing folder"))?,
            )?;
            tmp.write_all(content)?;
            tmp.as_file().sync_all()?;
            if expected.is_some() {
                tmp.persist(p)?;
            } else {
                tmp.persist_noclobber(p)?;
            }
            Ok(())
        })
    }
}
