#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SaveIntent {
    Save,
    SaveAs,
}

impl SaveIntent {
    pub(crate) fn uses_existing_path(self, has_existing_path: bool) -> bool {
        matches!(self, Self::Save) && has_existing_path
    }

    pub(crate) fn requires_overwrite_confirmation(
        self,
        has_existing_path: bool,
        destination_exists: bool,
    ) -> bool {
        destination_exists && !self.uses_existing_path(has_existing_path)
    }

    pub(crate) fn updates_persistent_path(self, has_existing_path: bool) -> bool {
        !self.uses_existing_path(has_existing_path)
    }
}

#[cfg(test)]
mod tests {
    use super::SaveIntent;

    #[test]
    fn normal_save_reuses_existing_native_path_without_confirmation() {
        assert!(SaveIntent::Save.uses_existing_path(true));
        assert!(!SaveIntent::Save.requires_overwrite_confirmation(true, true));
        assert!(!SaveIntent::Save.updates_persistent_path(true));
    }

    #[test]
    fn first_save_uses_picker_and_confirms_existing_destination() {
        assert!(!SaveIntent::Save.uses_existing_path(false));
        assert!(SaveIntent::Save.requires_overwrite_confirmation(false, true));
        assert!(SaveIntent::Save.updates_persistent_path(false));
    }

    #[test]
    fn save_as_always_uses_picker_and_confirms_existing_destination() {
        assert!(!SaveIntent::SaveAs.uses_existing_path(true));
        assert!(SaveIntent::SaveAs.requires_overwrite_confirmation(true, true));
        assert!(SaveIntent::SaveAs.updates_persistent_path(true));
    }

    #[test]
    fn newly_selected_nonexistent_destination_needs_no_confirmation() {
        assert!(!SaveIntent::Save.requires_overwrite_confirmation(false, false));
        assert!(!SaveIntent::SaveAs.requires_overwrite_confirmation(true, false));
    }
}
