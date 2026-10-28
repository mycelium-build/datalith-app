use super::*;

impl ThemeLibrary {
    pub fn update_color(&mut self, id: u64, token: &str, value: Option<String>) -> Result<u64> {
        self.update_document(id, |document| document.set_color(token, value))
    }

    pub fn update_highlight(&mut self, id: u64, token: &str, value: Option<String>) -> Result<u64> {
        self.update_document(id, |document| document.set_highlight(token, value))
    }

    /// Update a theme color or, for `highlight:` tokens, a syntax highlight color.
    pub fn update_token(&mut self, id: u64, token: &str, value: Option<String>) -> Result<u64> {
        match token.strip_prefix("highlight:") {
            Some(token) => self.update_highlight(id, token, value),
            None => self.update_color(id, token, value),
        }
    }

    pub fn update_font(&mut self, id: u64, role: FontRole, family: Option<String>) -> Result<u64> {
        self.update_document(id, |document| {
            document.set_font(role, family);
            Ok(())
        })
    }

    pub fn apply_fonts_to_all(&mut self, id: u64) -> Result<u64> {
        let family = self.variant_family_mut(id)?;
        let source = family.variant(id)?.document.clone();
        for variant in &mut family.variants {
            for role in FontRole::ALL {
                variant
                    .document
                    .set_font(role, source.font(role).map(str::to_owned));
            }
        }
        family.changed();
        Ok(family.id)
    }

    pub fn update_document(
        &mut self,
        variant_id: u64,
        change: impl FnOnce(&mut ThemeDocument) -> Result<()>,
    ) -> Result<u64> {
        let family = self.variant_family_mut(variant_id)?;
        let variant = family.variant_mut(variant_id)?;
        let mut next = variant.document.clone();
        change(&mut next)?;
        ensure!(
            next.name() == variant.name() && next.mode() == variant.mode(),
            "Use rename or change mode for identity changes"
        );
        variant.document = next;
        family.changed();
        Ok(family.id)
    }

    pub fn rename_family(&mut self, id: u64, name: &str) -> Result<()> {
        let old = self.editable_family(id)?.clone();
        let name = name.trim();
        self.unique_name(name, Some(id))?;
        let mut next = old.clone();
        next.name = name.into();
        for variant in &mut next.variants {
            let suffix = variant
                .name()
                .strip_prefix(&old.name)
                .unwrap_or_else(|| variant.name());
            variant.document.set_name(&format!("{name}{suffix}"));
        }
        self.unique_variant_names(next.variants.iter().map(ThemeVariant::name), Some(id))?;
        let mut slots = self.slots.clone();
        for (before, after) in old.variants.iter().zip(&next.variants) {
            rename_slot(&mut slots, before.name(), after.name());
        }
        self.commit_family(next, slots)
    }

    pub fn rename_variant(&mut self, id: u64, suffix: &str) -> Result<()> {
        let family = self.variant_family(id)?;
        let suffix = suffix.trim();
        ensure!(
            !suffix.is_empty() || family.variants.len() == 1,
            "Enter a variant name"
        );
        ensure!(
            !family.variants.iter().any(|v| v.id != id
                && v.name()
                    .eq_ignore_ascii_case(&format!("{} {suffix}", family.name))),
            "This variant name already exists"
        );
        let name = if suffix.is_empty() {
            family.name.clone()
        } else {
            format!("{} {suffix}", family.name)
        };
        self.unique_variant_names(std::iter::once(name.as_str()), Some(family.id))?;
        let mut slots = self.slots.clone();
        rename_slot(&mut slots, family.variant(id)?.name(), &name);
        let mut next = family.clone();
        next.variant_mut(id)?.document.set_name(&name);
        self.commit_family(next, slots)
    }

    /// The first addition supplies both suffixes atomically. For later additions `existing_suffix` is None.
    pub fn add_variant(
        &mut self,
        family_id: u64,
        from: u64,
        suffix: &str,
        existing_suffix: Option<&str>,
    ) -> Result<u64> {
        let family = self.editable_family(family_id)?;
        let source = family
            .variant(from)
            .context("Source variant is unavailable")?
            .clone();
        let suffix = suffix.trim();
        ensure!(!suffix.is_empty(), "Enter a variant name");
        if family.variants.len() == 1 {
            let existing = existing_suffix
                .context("Name both variants before adding")?
                .trim();
            ensure!(
                !existing.is_empty() && !existing.eq_ignore_ascii_case(suffix),
                "Variant suffixes must be non-empty and unique"
            );
            let first_name = format!("{} {existing}", family.name);
            let second_name = format!("{} {suffix}", family.name);
            self.unique_variant_names(
                [first_name.as_str(), second_name.as_str()],
                Some(family_id),
            )?;
            let mut next = family.clone();
            next.variants
                .first_mut()
                .context("Source variant is unavailable")?
                .document
                .set_name(&first_name);
            let id = next_id();
            let original_name = source.name().to_owned();
            let mut document = source.document;
            document.set_name(&second_name);
            next.variants.push(ThemeVariant { id, document });
            let mut slots = self.slots.clone();
            rename_slot(&mut slots, &original_name, &first_name);
            self.commit_family(next, slots)?;
            return Ok(id);
        }
        ensure!(
            !family.variants.iter().any(|v| family
                .suffix(v.id)
                .is_some_and(|s| s.eq_ignore_ascii_case(suffix))),
            "This variant name already exists"
        );
        let mut document = source.document;
        document.set_name(&format!("{} {suffix}", family.name));
        self.unique_variant_names(std::iter::once(document.name()), Some(family_id))?;
        let id = next_id();
        let family = self.editable_family_mut(family_id)?;
        family.variants.push(ThemeVariant { id, document });
        family.changed();
        Ok(id)
    }

    pub fn next_suffix(&self, family_id: u64) -> Result<String> {
        let family = self.family(family_id).context("Theme is unavailable")?;
        for n in 1.. {
            let suffix = format!("Variant {n}");
            if !family.variants.iter().any(|v| {
                family
                    .suffix(v.id)
                    .is_some_and(|s| s.eq_ignore_ascii_case(&suffix))
            }) {
                return Ok(suffix);
            }
        }
        bail!("No variant name available")
    }

    pub fn change_mode(&mut self, id: u64, mode: ThemeMode) -> Result<()> {
        let family = self.variant_family(id)?;
        let old = family.variant(id)?;
        if old.mode() == mode {
            return Ok(());
        }
        let old_name = old.name().to_owned();
        let old_mode = old.mode();
        let mut slots = self.slots.clone();
        let slot = slots.get_mut(ThemeKind::from(old_mode));
        if *slot == old_name {
            *slot = self.fallback_in_family(family, old_mode, &[id]);
        }
        let mut next = family.clone();
        next.variant_mut(id)?.document.set_mode(mode);
        self.commit_family(next, slots)
    }

    pub fn copy_family(&mut self, id: u64, name: &str) -> Result<u64> {
        self.unique_name(name, None)?;
        let mut set = self.family(id).context("Theme is unavailable")?.set();
        set.rename(name.trim());
        for variant in &mut set.themes {
            variant.config.is_default = false;
        }
        self.unique_variant_names(set.themes.iter().map(ThemeDocument::name), None)?;
        let path = self.new_path();
        storage::write(&path, &set)?;
        self.insert_set(set, ThemeSource::Custom(path))
    }

    /// The first free "{name} - Copy" name, then "{name} - Copy 2", and so on.
    pub fn suggested_copy_name(&self, name: &str) -> String {
        let mut candidate = format!("{name} - Copy");
        let mut n = 2u64;
        while self
            .families
            .iter()
            .any(|family| family.name.eq_ignore_ascii_case(&candidate))
        {
            candidate = format!("{name} - Copy {n}");
            n = n.saturating_add(1);
        }
        candidate
    }

    pub(in crate::app::themes) fn new_path(&self) -> PathBuf {
        self.directory
            .join(format!("theme-{:032x}.json", rand::random::<u128>()))
    }
}
