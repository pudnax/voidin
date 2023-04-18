use color_eyre::eyre::ContextCompat;
use color_eyre::{eyre::eyre, Result};
use pretty_type_name::pretty_type_name;
use std::any::Any;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::{
    any::TypeId,
    cell::{Ref, RefCell, RefMut},
};

use crate::Gpu;

pub trait Resource: 'static {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
}

impl<T: 'static> Resource for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

pub struct Read<'a, R: Resource>(pub(crate) Ref<'a, R>);

impl<R: Resource> Deref for Read<'_, R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<R: Resource> AsRef<R> for Read<'_, R> {
    fn as_ref(&self) -> &R {
        &self.0
    }
}

pub struct Write<'a, R: Resource>(pub(crate) RefMut<'a, R>);

impl<R: Resource> Deref for Write<'_, R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<R: Resource> DerefMut for Write<'_, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<R: Resource> AsMut<R> for Write<'_, R> {
    fn as_mut(&mut self) -> &mut R {
        &mut self.0
    }
}

impl<R: Resource> AsRef<R> for Write<'_, R> {
    fn as_ref(&self) -> &R {
        &self.0
    }
}

pub struct World {
    pub(crate) resources: HashMap<TypeId, RefCell<Box<dyn Resource>>>,
    pub gpu: Arc<Gpu>,
}

impl World {
    pub fn new(gpu: Arc<Gpu>) -> Self {
        Self {
            resources: HashMap::new(),
            gpu,
        }
    }

    pub fn insert<R: Resource>(&mut self, resource: R) {
        let id = TypeId::of::<R>();
        let returned = self.resources.insert(id, RefCell::new(Box::new(resource)));
        if returned.is_some() {
            let name = pretty_type_name::<R>();
            log::warn!("Replaced resource {} since it was already present", name);
        }
    }

    pub fn get<R: Resource>(&self) -> Result<Read<R>> {
        let cell = self
            .resources
            .get(&TypeId::of::<R>())
            .with_context(|| eyre!("Resource {} is not present", pretty_type_name::<R>()))?;
        let borrowed = cell.try_borrow()?;
        let borrowed = Ref::map(borrowed, |boxed| {
            boxed.as_ref().as_any().downcast_ref::<R>().unwrap()
        });
        Ok(Read(borrowed))
    }

    pub fn get_mut<R: Resource>(&self) -> Result<Write<R>> {
        let cell = self
            .resources
            .get(&TypeId::of::<R>())
            .with_context(|| eyre!("Resource {} is not present", pretty_type_name::<R>()))?;
        let borrowed = cell.try_borrow_mut()?;
        let borrowed = RefMut::map(borrowed, |boxed| {
            boxed.as_mut().as_any_mut().downcast_mut::<R>().unwrap()
        });
        Ok(Write(borrowed))
    }

    pub fn unwrap<R: Resource>(&self) -> Read<R> {
        self.get().unwrap()
    }

    pub fn unwrap_mut<R: Resource>(&self) -> Write<R> {
        self.get_mut().unwrap()
    }

    pub fn remove<R: Resource>(&mut self) -> Option<R> {
        self.resources.remove(&TypeId::of::<R>()).map(|cell| {
            let boxed = cell.into_inner();
            let any = boxed.into_any();
            let downcasted = any.downcast::<R>().unwrap();
            *downcasted
        })
    }

    pub fn contains<R: Resource>(&self) -> bool {
        self.resources.contains_key(&TypeId::of::<R>())
    }

    pub fn device(&self) -> &wgpu::Device {
        self.gpu.device()
    }

    pub fn queue(&self) -> &wgpu::Queue {
        self.gpu.queue()
    }
}
