//! Postgres-backed store: users/RBAC, lab ownership & sharing, firmware
//! metadata, and agent-session history.

use argon2::password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::model::*;
use crate::{DbError, Result};

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("src/migrations");

#[derive(Clone)]
pub struct Db {
    pool: PgPool,
}

impl Db {
    /// Connect, run migrations, and seed a default admin (admin/admin) when
    /// there are no users yet — logged loudly so it gets changed.
    pub async fn connect(url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(url)
            .await?;
        MIGRATOR.run(&pool).await?;
        let db = Self { pool };
        if db.user_count().await? == 0 {
            db.create_user("admin", "admin", Role::Admin).await?;
            tracing::warn!("seeded default admin user 'admin' / 'admin' — change this password");
        }
        Ok(db)
    }

    pub async fn user_count(&self) -> Result<i64> {
        let row = sqlx::query("SELECT count(*) AS n FROM users")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.get::<i64, _>("n"))
    }

    // ---- users / auth ------------------------------------------------------

    pub async fn create_user(&self, username: &str, password: &str, role: Role) -> Result<User> {
        let hash = hash_password(password)?;
        let id = Uuid::new_v4();
        let row = sqlx::query(
            "INSERT INTO users (id, username, role, password_hash) VALUES ($1,$2,$3,$4)
             ON CONFLICT (username) DO NOTHING
             RETURNING id, username, role, disabled, created_at",
        )
        .bind(id)
        .bind(username)
        .bind(role.as_str())
        .bind(hash)
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some(r) => Ok(user_from_row(&r)),
            None => Err(DbError::Conflict(format!("user '{username}' already exists"))),
        }
    }

    /// Verify username+password; returns the principal on success.
    pub async fn authenticate(&self, username: &str, password: &str) -> Result<Principal> {
        let row = sqlx::query(
            "SELECT id, username, role, password_hash, disabled FROM users WHERE username = $1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DbError::BadCredentials)?;

        if row.get::<bool, _>("disabled") {
            return Err(DbError::BadCredentials);
        }
        let stored: String = row.get("password_hash");
        verify_password(&stored, password)?;
        Ok(Principal {
            user_id: row.get("id"),
            username: row.get("username"),
            role: Role::parse(row.get::<&str, _>("role")).unwrap_or(Role::Viewer),
        })
    }

    pub async fn principal(&self, user_id: Uuid) -> Result<Principal> {
        let row = sqlx::query("SELECT id, username, role, disabled FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(DbError::NotFound)?;
        if row.get::<bool, _>("disabled") {
            return Err(DbError::NotFound);
        }
        Ok(Principal {
            user_id: row.get("id"),
            username: row.get("username"),
            role: Role::parse(row.get::<&str, _>("role")).unwrap_or(Role::Viewer),
        })
    }

    pub async fn list_users(&self) -> Result<Vec<User>> {
        let rows = sqlx::query(
            "SELECT id, username, role, disabled, created_at FROM users ORDER BY username",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(user_from_row).collect())
    }

    pub async fn set_password(&self, user_id: Uuid, password: &str) -> Result<()> {
        let hash = hash_password(password)?;
        sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
            .bind(hash)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_role(&self, user_id: Uuid, role: Role) -> Result<()> {
        sqlx::query("UPDATE users SET role = $1 WHERE id = $2")
            .bind(role.as_str())
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn find_user_by_name(&self, username: &str) -> Result<User> {
        let row = sqlx::query(
            "SELECT id, username, role, disabled, created_at FROM users WHERE username = $1",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DbError::NotFound)?;
        Ok(user_from_row(&row))
    }

    // ---- labs: ownership, visibility, sharing ------------------------------

    /// Record a newly created lab's owner. Idempotent on lab id.
    pub async fn register_lab(&self, lab_id: Uuid, owner: Uuid, name: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO labs (id, owner_id, name) VALUES ($1,$2,$3)
             ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, updated_at = now()",
        )
        .bind(lab_id)
        .bind(owner)
        .bind(name)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn forget_lab(&self, lab_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM labs WHERE id = $1")
            .bind(lab_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn lab_meta(&self, lab_id: Uuid) -> Result<Option<LabMeta>> {
        let row = sqlx::query(
            "SELECT l.id, l.owner_id, u.username AS owner_name, l.name, l.visibility
             FROM labs l JOIN users u ON u.id = l.owner_id WHERE l.id = $1",
        )
        .bind(lab_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| LabMeta {
            id: r.get("id"),
            owner_id: r.get("owner_id"),
            owner_name: r.get("owner_name"),
            name: r.get("name"),
            visibility: Visibility::parse(r.get::<&str, _>("visibility")),
        }))
    }

    pub async fn set_visibility(&self, lab_id: Uuid, v: Visibility) -> Result<()> {
        sqlx::query("UPDATE labs SET visibility = $1, updated_at = now() WHERE id = $2")
            .bind(v.as_str())
            .bind(lab_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Resolve a principal's effective access to a lab (RBAC + ownership +
    /// visibility + explicit shares). Admins own everything.
    pub async fn access_for(&self, p: &Principal, lab_id: Uuid) -> Result<Access> {
        if p.role.is_admin() {
            return Ok(Access::Own);
        }
        let Some(meta) = self.lab_meta(lab_id).await? else {
            // Unregistered labs (pre-existing / imported without a DB owner)
            // are readable by any authenticated user, editable by writers.
            return Ok(if p.role.can_write() { Access::Edit } else { Access::View });
        };
        if meta.owner_id == p.user_id {
            return Ok(Access::Own);
        }
        // An explicit share grant is honored as-is and wins over the
        // visibility default: a per-lab "edit" grant elevates even a global
        // viewer on that one lab (they still can't *create* labs — the
        // Writer extractor gates that).
        let row = sqlx::query("SELECT access FROM lab_shares WHERE lab_id = $1 AND user_id = $2")
            .bind(lab_id)
            .bind(p.user_id)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(r) = row {
            return Ok(Access::parse(r.get::<&str, _>("access")));
        }
        if meta.visibility == Visibility::Public {
            return Ok(Access::View);
        }
        Ok(Access::None)
    }

    /// Lab ids visible to a principal (owner, shared, or public; all for admin).
    pub async fn visible_lab_ids(&self, p: &Principal) -> Result<Vec<Uuid>> {
        if p.role.is_admin() {
            let rows = sqlx::query("SELECT id FROM labs").fetch_all(&self.pool).await?;
            return Ok(rows.iter().map(|r| r.get::<Uuid, _>("id")).collect());
        }
        let rows = sqlx::query(
            "SELECT DISTINCT l.id FROM labs l
             LEFT JOIN lab_shares s ON s.lab_id = l.id AND s.user_id = $1
             WHERE l.owner_id = $1 OR l.visibility = 'public' OR s.user_id IS NOT NULL",
        )
        .bind(p.user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| r.get::<Uuid, _>("id")).collect())
    }

    pub async fn share_lab(&self, lab_id: Uuid, user_id: Uuid, access: Access) -> Result<()> {
        sqlx::query(
            "INSERT INTO lab_shares (lab_id, user_id, access) VALUES ($1,$2,$3)
             ON CONFLICT (lab_id, user_id) DO UPDATE SET access = EXCLUDED.access",
        )
        .bind(lab_id)
        .bind(user_id)
        .bind(access.as_str())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn unshare_lab(&self, lab_id: Uuid, user_id: Uuid) -> Result<()> {
        sqlx::query("DELETE FROM lab_shares WHERE lab_id = $1 AND user_id = $2")
            .bind(lab_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn shares_for(&self, lab_id: Uuid) -> Result<Vec<ShareGrant>> {
        let rows = sqlx::query(
            "SELECT s.user_id, u.username, s.access FROM lab_shares s
             JOIN users u ON u.id = s.user_id WHERE s.lab_id = $1 ORDER BY u.username",
        )
        .bind(lab_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| ShareGrant {
                user_id: r.get("user_id"),
                username: r.get("username"),
                access: Access::parse(r.get::<&str, _>("access")),
            })
            .collect())
    }

    // ---- firmware metadata -------------------------------------------------

    pub async fn record_firmware(
        &self,
        template: &str,
        version: &str,
        filename: &str,
        size_bytes: i64,
        sha256: Option<&str>,
        uploaded_by: Option<Uuid>,
    ) -> Result<FirmwareImage> {
        let id = Uuid::new_v4();
        let row = sqlx::query(
            "INSERT INTO firmware_images (id, template, version, filename, size_bytes, sha256, uploaded_by)
             VALUES ($1,$2,$3,$4,$5,$6,$7)
             ON CONFLICT (template, version) DO UPDATE
               SET filename = EXCLUDED.filename, size_bytes = EXCLUDED.size_bytes,
                   sha256 = EXCLUDED.sha256, uploaded_by = EXCLUDED.uploaded_by, uploaded_at = now()
             RETURNING id, template, version, filename, size_bytes, sha256, uploaded_by, uploaded_at",
        )
        .bind(id)
        .bind(template)
        .bind(version)
        .bind(filename)
        .bind(size_bytes)
        .bind(sha256)
        .bind(uploaded_by)
        .fetch_one(&self.pool)
        .await?;
        Ok(firmware_from_row(&row))
    }

    pub async fn list_firmware(&self) -> Result<Vec<FirmwareImage>> {
        let rows = sqlx::query(
            "SELECT id, template, version, filename, size_bytes, sha256, uploaded_by, uploaded_at
             FROM firmware_images ORDER BY template, version",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(firmware_from_row).collect())
    }

    pub async fn delete_firmware(&self, template: &str, version: &str) -> Result<()> {
        sqlx::query("DELETE FROM firmware_images WHERE template = $1 AND version = $2")
            .bind(template)
            .bind(version)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ---- agent sessions ----------------------------------------------------

    pub async fn create_agent_session(
        &self,
        lab_id: Uuid,
        user_id: Uuid,
        title: &str,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        sqlx::query("INSERT INTO agent_sessions (id, lab_id, user_id, title) VALUES ($1,$2,$3,$4)")
            .bind(id)
            .bind(lab_id)
            .bind(user_id)
            .bind(title)
            .execute(&self.pool)
            .await?;
        Ok(id)
    }

    /// Append one transcript item; returns the assigned sequence number.
    pub async fn append_agent_event(
        &self,
        session_id: Uuid,
        payload: &serde_json::Value,
    ) -> Result<i32> {
        let row = sqlx::query(
            "INSERT INTO agent_events (session_id, seq, payload)
             VALUES ($1, (SELECT COALESCE(MAX(seq)+1, 0) FROM agent_events WHERE session_id = $1), $2)
             RETURNING seq",
        )
        .bind(session_id)
        .bind(payload)
        .fetch_one(&self.pool)
        .await?;
        sqlx::query("UPDATE agent_sessions SET updated_at = now() WHERE id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(row.get::<i32, _>("seq"))
    }

    pub async fn agent_events(&self, session_id: Uuid) -> Result<Vec<serde_json::Value>> {
        let rows = sqlx::query(
            "SELECT payload FROM agent_events WHERE session_id = $1 ORDER BY seq",
        )
        .bind(session_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| r.get::<serde_json::Value, _>("payload")).collect())
    }

    /// Sessions for a lab visible to a user (their own; admins see all).
    pub async fn list_agent_sessions(
        &self,
        lab_id: Uuid,
        p: &Principal,
    ) -> Result<Vec<AgentSessionMeta>> {
        let base = "SELECT s.id, s.lab_id, s.user_id, s.title, s.updated_at,
                    (SELECT count(*) FROM agent_events e WHERE e.session_id = s.id) AS n
                    FROM agent_sessions s WHERE s.lab_id = $1";
        let rows = if p.role.is_admin() {
            sqlx::query(&format!("{base} ORDER BY s.updated_at DESC"))
                .bind(lab_id)
                .fetch_all(&self.pool)
                .await?
        } else {
            sqlx::query(&format!("{base} AND s.user_id = $2 ORDER BY s.updated_at DESC"))
                .bind(lab_id)
                .bind(p.user_id)
                .fetch_all(&self.pool)
                .await?
        };
        Ok(rows
            .iter()
            .map(|r| AgentSessionMeta {
                id: r.get("id"),
                lab_id: r.get("lab_id"),
                user_id: r.get("user_id"),
                title: r.get("title"),
                updated_at: r.get("updated_at"),
                event_count: r.get::<i64, _>("n"),
            })
            .collect())
    }

    pub async fn session_owner(&self, session_id: Uuid) -> Result<Uuid> {
        let row = sqlx::query("SELECT user_id FROM agent_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(DbError::NotFound)?;
        Ok(row.get("user_id"))
    }

    // ---- audit -------------------------------------------------------------

    pub async fn audit(&self, user: Option<Uuid>, action: &str, target: Option<&str>, detail: Option<&str>) {
        let _ = sqlx::query(
            "INSERT INTO audit_log (user_id, action, target, detail) VALUES ($1,$2,$3,$4)",
        )
        .bind(user)
        .bind(action)
        .bind(target)
        .bind(detail)
        .execute(&self.pool)
        .await;
    }
}

// ---- row mappers & password helpers ---------------------------------------

fn user_from_row(r: &sqlx::postgres::PgRow) -> User {
    User {
        id: r.get("id"),
        username: r.get("username"),
        role: Role::parse(r.get::<&str, _>("role")).unwrap_or(Role::Viewer),
        disabled: r.get("disabled"),
        created_at: r.get("created_at"),
    }
}

fn firmware_from_row(r: &sqlx::postgres::PgRow) -> FirmwareImage {
    FirmwareImage {
        id: r.get("id"),
        template: r.get("template"),
        version: r.get("version"),
        filename: r.get("filename"),
        size_bytes: r.get("size_bytes"),
        sha256: r.get("sha256"),
        uploaded_by: r.get("uploaded_by"),
        uploaded_at: r.get("uploaded_at"),
    }
}

fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| DbError::Hash)
}

fn verify_password(stored: &str, password: &str) -> Result<()> {
    let parsed = PasswordHash::new(stored).map_err(|_| DbError::Hash)?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| DbError::BadCredentials)
}
