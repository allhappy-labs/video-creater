//! Gives an ignored Secret Service test its own GNOME Keyring collection.
//!
//! The provider tests change keyring state: one locks the collection new
//! credentials are written to. Each test instead points the `default` alias at a
//! fresh, unlocked collection and restores the previous alias when it ends, so
//! the tests pass in any order and in one process.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

const SECRETS_BUS_NAME: &str = "org.freedesktop.secrets";
const SECRETS_PATH: &str = "/org/freedesktop/secrets";
const SERVICE_INTERFACE: &str = "org.freedesktop.Secret.Service";
const COLLECTION_INTERFACE: &str = "org.freedesktop.Secret.Collection";
/// GNOME Keyring's interface for creating a collection without a GUI prompt.
const GNOME_KEYRING_INTERNAL_INTERFACE: &str =
    "org.gnome.keyring.InternalUnsupportedGuiltRiddenInterface";

/// The `default` alias is shared by the whole provider, so the tests take turns.
static DEFAULT_ALIAS: Mutex<()> = Mutex::new(());

pub struct DisposableDefaultCollection {
    connection: Connection,
    previous_default: OwnedObjectPath,
    collection: OwnedObjectPath,
    _alias: MutexGuard<'static, ()>,
}

impl DisposableDefaultCollection {
    pub fn create() -> Self {
        let alias = DEFAULT_ALIAS
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let connection = Connection::session().expect("session bus");
        let service = proxy(&connection, SECRETS_PATH, SERVICE_INTERFACE);
        let previous_default: OwnedObjectPath = service
            .call("ReadAlias", &("default",))
            .expect("read the default alias");
        let (_, session): (OwnedValue, OwnedObjectPath) = service
            .call("OpenSession", &("plain", Value::from("")))
            .expect("open a plain Secret Service session");
        let label = format!("video-creater-test-{}", uuid::Uuid::new_v4());
        let properties = HashMap::from([(
            "org.freedesktop.Secret.Collection.Label",
            Value::from(label.as_str()),
        )]);
        let master_password = (
            session,
            Vec::<u8>::new(),
            b"disposable".to_vec(),
            "text/plain",
        );
        let collection: OwnedObjectPath =
            proxy(&connection, SECRETS_PATH, GNOME_KEYRING_INTERNAL_INTERFACE)
                .call("CreateWithMasterPassword", &(properties, master_password))
                .expect("create a disposable collection (needs GNOME Keyring)");
        service
            .call::<_, _, ()>("SetAlias", &("default", &collection))
            .expect("point the default alias at the disposable collection");
        Self {
            connection,
            previous_default,
            collection,
            _alias: alias,
        }
    }
}

impl Drop for DisposableDefaultCollection {
    fn drop(&mut self) {
        let _ = proxy(&self.connection, SECRETS_PATH, SERVICE_INTERFACE)
            .call::<_, _, ()>("SetAlias", &("default", &self.previous_default));
        let _ = proxy(
            &self.connection,
            self.collection.as_str(),
            COLLECTION_INTERFACE,
        )
        .call::<_, _, OwnedObjectPath>("Delete", &());
    }
}

fn proxy<'a>(connection: &Connection, path: &'a str, interface: &'a str) -> Proxy<'a> {
    Proxy::new(connection, SECRETS_BUS_NAME, path, interface).expect("Secret Service proxy")
}
