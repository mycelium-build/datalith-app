use super::*;

impl ThemeLibrary {
    pub fn export(&self, id: u64, to: &Path) -> Result<()> {
        let family = self.family(id).context("Theme is unavailable")?;
        ensure!(family.is_custom(), "Only custom themes can be exported");
        storage::write(to, &family.set())
    }

    /// Import a theme file as a new custom family; a taken name becomes "<name> - Copy".
    pub fn import(&mut self, from: &Path) -> Result<u64> {
        let mut set = storage::read(from)?;
        if self
            .families
            .iter()
            .any(|family| family.name.eq_ignore_ascii_case(&set.name))
        {
            set.rename(&self.suggested_copy_name(&set.name));
        }
        self.unique_variant_names(set.themes.iter().map(ThemeDocument::name), None)?;
        self.store_import(set)
    }

    fn store_import(&mut self, set: StoredThemeSet) -> Result<u64> {
        let path = self.new_path();
        let id = self.insert_set(set, ThemeSource::Custom(path.clone()))?;
        let normalized = self
            .family(id)
            .context("Imported theme is unavailable")?
            .set();
        if let Err(error) = storage::write(&path, &normalized) {
            self.families.retain(|family| family.id != id);
            return Err(error);
        }
        Ok(id)
    }

    /// Flush the latest revision for one family. A failed write retains the model and exposes Retry.
    pub fn flush_family(&mut self, id: u64) -> Result<()> {
        let family = self.editable_family_mut(id)?;
        let ThemeSource::Custom(ref path) = family.source else {
            bail!("Built-in themes cannot be saved")
        };
        let result = storage::write(path, &family.set());
        family.status = match &result {
            Ok(()) => SaveStatus::Autosaved,
            Err(error) => SaveStatus::Failed(error.to_string()),
        };
        result
    }

    pub fn flush_pending(&mut self) -> Vec<(u64, anyhow::Error)> {
        let mut errors = Vec::new();
        for ix in 0..self.families.len() {
            if let Some(family) = self.families.get(ix)
                && matches!(family.status, SaveStatus::Saving | SaveStatus::Failed(_))
            {
                let id = family.id;
                if let Err(error) = self.flush_family(id) {
                    errors.push((id, error));
                }
            }
        }
        errors
    }

    /// Schedule after each valid edit; revision checks coalesce edits and prevent stale writes.
    pub fn schedule_save(id: u64, cx: &App) {
        let revision = cx.global::<Self>().family(id).map(|f| f.revision);
        cx.spawn(async move |cx| {
            cx.background_executor()
                .timer(Duration::from_millis(400))
                .await;
            cx.update(|cx| {
                if cx.try_global::<Self>().is_some() {
                    let library = cx.global_mut::<Self>();
                    if library.family(id).is_some_and(|f| {
                        Some(f.revision) == revision && matches!(f.status, SaveStatus::Saving)
                    }) {
                        let _ = library.flush_family(id);
                        cx.refresh_windows();
                    }
                }
            });
        })
        .detach();
    }
}
