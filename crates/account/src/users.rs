//! The people directory. Listing never changes an account; writes stay with the
//! account and board rules that already own them.
use crate::{Error, Role, unavailable};
use postgres::GenericClient;
use uuid::Uuid;

/// One search returns at most this many people. The directory is a local
/// workspace list, not a paged remote API.
pub const DIRECTORY_LIMIT: i64 = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserSummary {
    pub id: Uuid,
    pub username: String,
    pub email: String,
    pub role: Role,
    pub active: bool,
    pub teams: Vec<String>,
}

/// Blank input lists everyone; otherwise a case-insensitive substring of the
/// username or email matches. The search text is data, never SQL.
pub fn search(db: &mut impl GenericClient, query: &str) -> Result<Vec<UserSummary>, Error> {
    let query = query.trim();
    if query.encode_utf16().take(321).count() > 320 || query.contains('\0') {
        return Ok(Vec::new());
    }
    let rows = db
        .query(
            "SELECT a.id, a.username, a.email, a.role, a.active,
                    coalesce(array_agg(t.name ORDER BY lower(t.name), t.id)
                        FILTER (WHERE t.id IS NOT NULL), '{}')
             FROM accounts a
             LEFT JOIN team_members m ON m.account_id = a.id
             LEFT JOIN teams t ON t.id = m.team_id
             WHERE $1 = '' OR strpos(lower(a.username), lower($1)) > 0
                OR strpos(lower(a.email), lower($1)) > 0
             GROUP BY a.id
             ORDER BY lower(a.username), a.id
             LIMIT $2",
            &[&query, &DIRECTORY_LIMIT],
        )
        .map_err(unavailable("list users"))?;
    rows.into_iter()
        .map(|row| {
            let role = match row.get::<_, &str>(3) {
                "viewer" => Role::Viewer,
                "member" => Role::Member,
                "owner" => Role::Owner,
                _ => return Err(unavailable("list users")(())),
            };
            Ok(UserSummary {
                id: row.get(0),
                username: row.get(1),
                email: row.get(2),
                role,
                active: row.get(4),
                teams: row.get(5),
            })
        })
        .collect()
}
