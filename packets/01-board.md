# 01-board

Add the board model to the existing Rust workspace. On a synthetic board, create two uniquely identified items with the same title. Rename one by identity: its order and the other item must not change. Editing a missing item returns a recoverable outcome and leaves the board intact. Cover both paths in the `board_edit` integration test. No database or renderer needed for these rules.
