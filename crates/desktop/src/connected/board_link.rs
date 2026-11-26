use uuid::Uuid;

pub(super) fn address(port: u16, board: Uuid) -> String {
    format!("http://127.0.0.1:{port}/boards/{board}")
}

/// Reduce text to an ID, never a host or a URL to request. The round trip also
/// excludes UUID aliases, escaping and every noncanonical path suffix.
pub(super) fn parse(port: u16, text: &str) -> Option<Uuid> {
    let id = Uuid::parse_str(text.rsplit_once('/')?.1).ok()?;
    (!id.is_nil() && text == address(port, id)).then_some(id)
}

// Kit's clipboard setter has no error result. Use the OS acknowledgement;
// never read back the clipboard to guess whether this write succeeded.
#[cfg(windows)]
pub(super) fn copy(text: &str) -> windows::core::Result<()> {
    use windows::ApplicationModel::DataTransfer::{Clipboard, DataPackage};
    let data = DataPackage::new()?;
    data.SetText(&text.into())?;
    Clipboard::SetContent(&data)?;
    Clipboard::Flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_a_canonical_board_at_the_connected_origin() {
        let board = Uuid::from_u128(201);
        let valid = address(4321, board);
        assert_eq!(parse(4321, &valid), Some(board));
        for invalid in [
            address(4322, board),
            valid.replace("127.0.0.1", "localhost"),
            valid.replace("127.0.0.1", "user@127.0.0.1"),
            valid.replace("http:", "https:"),
            format!("{valid}/items"),
            format!("{valid}?"),
            format!("{valid}#view"),
            valid.replace("/boards/", "/navigation/boards/"),
            valid.replace('-', ""),
            valid.to_uppercase(),
            address(4321, Uuid::nil()),
            format!("{valid} "),
            "http://127.0.0.1:4321/boards/not-a-uuid".into(),
        ] {
            assert!(parse(4321, &invalid).is_none());
        }
    }
}
