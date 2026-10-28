use super::*;

impl ThemeLibrary {
    pub fn remove_variant(&mut self, id: u64) -> Result<DeletedTheme> {
        let family = self.variant_family(id)?;
        if family.variants.len() == 1 {
            return self.delete_family(family.id);
        }
        let old = family.clone();
        let before = self.slots.clone();
        let mut next = old.clone();
        let position = next
            .variants
            .iter()
            .position(|v| v.id == id)
            .context("Theme variant is unavailable")?;
        let removed = next.variants.remove(position);
        let mut slots = self.slots.clone();
        let slot = slots.get_mut(ThemeKind::from(removed.mode()));
        if *slot == removed.name() {
            *slot = self.fallback_in_family(family, removed.mode(), &[id]);
        }
        let position = self
            .families
            .iter()
            .position(|family| family.id == old.id)
            .context("Theme is unavailable")?;
        self.commit_family(next, slots)?;
        Ok(DeletedTheme {
            old,
            position,
            slots_before: before,
            selection_revisions: self.selection_revisions,
            removed_variant_id: Some(removed.id),
        })
    }

    pub fn delete_family(&mut self, id: u64) -> Result<DeletedTheme> {
        let (ix, family) = self
            .families
            .iter()
            .enumerate()
            .find(|(_, family)| family.id == id)
            .context("Theme is unavailable")?;
        family.ensure_editable()?;
        let old = family.clone();
        let before = self.slots.clone();
        let mut slots = before.clone();
        let ids: Vec<_> = old.variants.iter().map(|v| v.id).collect();
        for kind in ThemeKind::ALL {
            let slot = slots.get_mut(kind);
            if old.variants.iter().any(|v| v.name() == *slot) {
                *slot = self.fallback(kind.mode(), &ids);
            }
        }
        if let ThemeSource::Custom(ref path) = old.source {
            storage::remove_and_update(path, || persist_slots(&slots))?;
        }
        self.slots = slots;
        self.families.remove(ix);
        Ok(DeletedTheme {
            old,
            position: ix,
            slots_before: before,
            selection_revisions: self.selection_revisions,
            removed_variant_id: None,
        })
    }

    pub fn undo_delete(&mut self, deleted: DeletedTheme) -> Result<()> {
        let removed_family = deleted.removed_variant_id.is_none();
        self.unique_name(
            &deleted.old.name,
            (!removed_family).then_some(deleted.old.id),
        )?;
        let path = match &deleted.old.source {
            ThemeSource::Custom(path) => path.clone(),
            ThemeSource::Bundled => bail!("Built-in themes cannot be restored"),
        };
        let restore_slots = ThemeModes::from_fn(|kind| {
            self.selection_revisions.get(kind) == deleted.selection_revisions.get(kind)
                && deleted.old.variants.iter().any(|variant| {
                    variant.name() == deleted.slots_before.get(kind).as_str()
                        && variant.mode() == kind.mode()
                        && deleted.removed_variant_id.is_none_or(|id| variant.id == id)
                })
        });
        let mut next = if let Some(removed_id) = deleted.removed_variant_id {
            let current = self
                .family(deleted.old.id)
                .context("Theme was renamed or deleted after removing the variant")?;
            Self::restore_variant(current, &deleted.old, removed_id)?
        } else {
            deleted.old
        };
        let mut slots = self.slots.clone();
        for kind in ThemeKind::ALL {
            if *restore_slots.get(kind) {
                slots
                    .get_mut(kind)
                    .clone_from(deleted.slots_before.get(kind));
            }
        }
        self.validate_restored(&next, &slots, !removed_family)?;
        if !removed_family {
            return self.commit_family(next, slots);
        }
        storage::write(&path, &next.set())?;
        if let Err(error) = persist_slots(&slots) {
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
        next.persisted();
        self.families
            .insert(deleted.position.min(self.families.len()), next);
        self.slots = slots;
        Ok(())
    }

    fn restore_variant(current: &ThemeFamily, old: &ThemeFamily, id: u64) -> Result<ThemeFamily> {
        ensure!(
            current.name == old.name,
            "Theme was renamed after removing the variant"
        );
        let (position, removed) = old
            .variants
            .iter()
            .enumerate()
            .find(|(_, variant)| variant.id == id)
            .context("Removed variant is unavailable")?;
        ensure!(
            !current.variants.iter().any(|variant| variant.id == id),
            "Variant has already been restored"
        );
        let mut next = current.clone();
        let successor = old
            .variants
            .iter()
            .skip(position.saturating_add(1))
            .find_map(|variant| next.variants.iter().position(|v| v.id == variant.id));
        let predecessor = || {
            old.variants
                .iter()
                .take(position)
                .rev()
                .find_map(|variant| {
                    next.variants
                        .iter()
                        .position(|v| v.id == variant.id)
                        .map(|ix| ix.saturating_add(1))
                })
        };
        let index = successor
            .or_else(predecessor)
            .unwrap_or_else(|| position.min(next.variants.len()));
        next.variants.insert(index, removed.clone());
        Ok(next)
    }

    fn validate_restored(
        &self,
        family: &ThemeFamily,
        slots: &ThemeModes<String>,
        replacing: bool,
    ) -> Result<()> {
        ensure!(
            !family.variants.is_empty(),
            "A theme needs at least one variant"
        );
        ensure!(
            family.variants.iter().all(|variant| valid_variant_name(
                &family.name,
                variant.name(),
                family.variants.len() > 1
            )),
            "Restored theme has invalid variant names"
        );
        self.unique_variant_names(
            family.variants.iter().map(ThemeVariant::name),
            replacing.then_some(family.id),
        )?;
        for kind in ThemeKind::ALL {
            let name = slots.get(kind);
            let valid_in_restored = family
                .variants
                .iter()
                .any(|variant| variant.name() == name && variant.mode() == kind.mode());
            let valid_elsewhere = self
                .families
                .iter()
                .filter(|other| other.id != family.id)
                .flat_map(|other| &other.variants)
                .any(|variant| variant.name() == name && variant.mode() == kind.mode());
            ensure!(
                valid_in_restored || valid_elsewhere,
                "Current {kind:?} theme is unavailable or has the wrong mode"
            );
        }
        Ok(())
    }
}
