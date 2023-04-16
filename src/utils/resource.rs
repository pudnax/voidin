use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::Arc,
};

use parking_lot::RwLock;

use crate::Gpu;

pub trait Resource: Sized {
    fn init(gpu: Arc<Gpu>) -> Self;
}

#[derive(Clone)]
pub struct Ref<T>(Arc<RwLock<T>>)
where
    T: ?Sized;

impl<T: Resource> Ref<T> {
    pub fn get(&self) -> impl std::ops::Deref<Target = T> + '_ {
        self.0.read()
    }

    pub fn get_mut(&self) -> impl std::ops::DerefMut<Target = T> + '_ {
        self.0.write()
    }
}

pub struct World {
    gpu: Arc<Gpu>,
    resources: RwLock<HashMap<TypeId, Arc<dyn Any + Send + Sync>>>,
}

impl World {
    pub fn new(gpu: Arc<Gpu>) -> Self {
        Self {
            gpu,
            resources: Default::default(),
        }
    }

    pub fn get<T>(&self) -> Ref<T>
    where
        T: Resource + Send + Sync + 'static,
    {
        let read = self.resources.read();

        let arc = match read.get(&TypeId::of::<T>()) {
            Some(arc) => arc.clone(),
            None => {
                drop(read);
                self.resources
                    .write()
                    .entry(TypeId::of::<T>())
                    .or_insert_with(|| {
                        let ressource = <T as Resource>::init(self.gpu.clone());
                        Arc::new(RwLock::new(ressource))
                    })
                    .clone()
            }
        };

        Ref(arc.downcast::<RwLock<T>>().unwrap())
    }
}
