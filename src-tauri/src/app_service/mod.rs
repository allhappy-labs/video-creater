pub mod agents;
pub mod context;
pub mod error;
pub mod events;
pub mod exports;
pub mod jobs;
pub mod media;
pub mod operation;
pub mod projects;

use std::sync::Arc;

use events::EventSink;

pub trait ProjectStore: Send + Sync {
    fn name(&self) -> &'static str;
}

pub trait JobCoordinator: Send + Sync {
    fn name(&self) -> &'static str;
}

pub trait CredentialStore: Send + Sync {
    fn name(&self) -> &'static str;
}

pub trait AgentCoordinator: Send + Sync {
    fn name(&self) -> &'static str;
}

pub trait RenderCoordinator: Send + Sync {
    fn name(&self) -> &'static str;
}

#[derive(Clone)]
pub struct ServiceDependencies {
    pub project_store: Arc<dyn ProjectStore>,
    pub jobs: Arc<dyn JobCoordinator>,
    pub credentials: Arc<dyn CredentialStore>,
    pub agents: Arc<dyn AgentCoordinator>,
    pub renders: Arc<dyn RenderCoordinator>,
}

impl ServiceDependencies {
    pub fn unconfigured() -> Self {
        Self {
            project_store: Arc::new(Unconfigured("unconfigured-project-store")),
            jobs: Arc::new(Unconfigured("unconfigured-job-coordinator")),
            credentials: Arc::new(Unconfigured("unconfigured-credential-store")),
            agents: Arc::new(Unconfigured("unconfigured-agent-coordinator")),
            renders: Arc::new(Unconfigured("unconfigured-render-coordinator")),
        }
    }
}

struct Unconfigured(&'static str);

macro_rules! impl_unconfigured_component {
    ($trait_name:ident) => {
        impl $trait_name for Unconfigured {
            fn name(&self) -> &'static str {
                self.0
            }
        }
    };
}

impl_unconfigured_component!(ProjectStore);
impl_unconfigured_component!(JobCoordinator);
impl_unconfigured_component!(CredentialStore);
impl_unconfigured_component!(AgentCoordinator);
impl_unconfigured_component!(RenderCoordinator);

#[derive(Clone)]
pub struct VideoCreaterService {
    dependencies: ServiceDependencies,
    events: Arc<dyn EventSink>,
}

impl VideoCreaterService {
    pub fn new(dependencies: ServiceDependencies, events: Arc<dyn EventSink>) -> Self {
        Self {
            dependencies,
            events,
        }
    }

    pub fn dependencies(&self) -> &ServiceDependencies {
        &self.dependencies
    }

    pub fn events(&self) -> &Arc<dyn EventSink> {
        &self.events
    }
}
