use uuid::Uuid;

pub mod store;
pub mod views;

macro_rules! record_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $name(Uuid);

        impl $name {
            pub const fn new(id: Uuid) -> Option<Self> {
                if id.is_nil() { None } else { Some(Self(id)) }
            }

            pub const fn as_uuid(self) -> Uuid {
                self.0
            }
        }
    };
}

record_id!(BoardId);
record_id!(ItemId);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub id: ItemId,
    pub title: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct DuplicateItem(pub ItemId);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenameError {
    BlankTitle,
    TitleTooLong,
    MissingItem(ItemId),
}

/// Only direct edits use this rule; loading an existing title preserves its spelling.
pub fn interactive_title(draft: &str) -> Result<&str, RenameError> {
    let title = draft.trim();
    if title.is_empty() {
        Err(RenameError::BlankTitle)
    } else if title.encode_utf16().nth(255).is_some() {
        Err(RenameError::TitleTooLong)
    } else {
        Ok(title)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Board {
    id: BoardId,
    items: Vec<Item>,
}

impl Board {
    pub fn new(id: BoardId) -> Self {
        Self { id, items: vec![] }
    }

    pub fn id(&self) -> BoardId {
        self.id
    }

    /// Items in board order.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn add_item(&mut self, id: ItemId, title: impl Into<String>) -> Result<(), DuplicateItem> {
        if self.items.iter().any(|item| item.id == id) {
            return Err(DuplicateItem(id));
        }
        self.items.push(Item {
            id,
            title: title.into(),
        });
        Ok(())
    }

    pub fn rename_item(&mut self, id: ItemId, draft: &str) -> Result<(), RenameError> {
        let title = interactive_title(draft)?;
        let item = self
            .items
            .iter_mut()
            .find(|item| item.id == id)
            .ok_or(RenameError::MissingItem(id))?;
        item.title = title.into();
        Ok(())
    }
}
