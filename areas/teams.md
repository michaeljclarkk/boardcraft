---
id: teams
name: Teams and users
order: 3
percent: 5
code: crates/account/src/users, crates/account/src/teams
---
# Teams and users

Users, roles, teams and membership: create and manage accounts, assign roles, add people to teams and keep access rules consistent with board permissions.

## Workflows

- [x] List and search users
- [ ] Create, rename and deactivate a user
- [ ] Assign a role and team
- [ ] Add or remove a team member
- [ ] See the effective access for a user

## Notes

The Windows browse board list opens a People page backed by the account directory rule and one host route. It lists every account with username, email, role, active state and team names, and filters on a username/email substring; Computer Use verified both. The teams table and membership relation exist but no management workflow is ported. Browser, Android, account writes and effective-access inspection remain open.
