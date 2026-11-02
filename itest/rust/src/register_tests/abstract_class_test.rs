/*
 * Copyright (c) godot-rust; Bromeon and contributors.
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! Tests for `#[class(abstract)]`, `#[func(abstract)]` and `#[class(base = SomeRustClass)]`.

use godot::builtin::{Array, VarDictionary, varray};
use godot::classes::{ClassDb, Node, Resource};
use godot::meta::ToGodot;
use godot::obj::{Gd, NewAlloc, NewGd};
use godot::prelude::*;
use godot::register::info::MethodFlags;

use crate::framework::{expect_panic, itest};

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Declarations: abstract base class, two concrete subclasses

/// Abstract base class: cannot be instantiated, holds instance state, declares concrete and abstract methods.
#[derive(GodotClass)]
#[class(abstract, base = Resource)]
pub struct AbstractShape {
    base: Base<Resource>,

    /// Base-class state, inherited by every subclass.
    #[var]
    #[init(val = 1.0)]
    scale: f64,

    #[var]
    label: GString,
}

#[godot_api]
impl AbstractShape {
    /// Area of the shape.
    #[func(abstract)]
    fn area(&self) -> f64;

    #[func(abstract)]
    fn scaled_name(&mut self, factor: i64) -> GString;

    /// Concrete base method, operating on base-class state. Inherited by every subclass.
    #[func]
    fn describe(&self) -> GString {
        GString::from(&format!("{} @{}", self.label, self.scale))
    }

    /// Concrete base method with `&mut self`.
    #[func]
    fn grow(&mut self, by: f64) -> f64 {
        self.scale += by;
        self.scale
    }

    /// Base methods can use the inherited `Base<T>` handle.
    #[func]
    fn base_class_name(&self) -> GString {
        GString::from(&self.base().get_class().to_string())
    }

    /// Statics are fine on abstract classes: they ignore the instance pointer.
    #[func]
    fn describe_kind() -> GString {
        "shape".into()
    }

    #[constant]
    const SIDES_UNKNOWN: i64 = -1;
}

#[derive(GodotClass)]
#[class(init, base = AbstractShape)]
pub struct Circle {
    shape: AbstractShape,

    #[var]
    radius: f64,
}

#[godot_api]
impl Circle {
    #[func]
    fn circumference(&self) -> f64 {
        2.0 * std::f64::consts::PI * self.radius
    }

    /// Rust-side access to the base class's state.
    #[func]
    fn scaled_area(&self) -> f64 {
        self.area() * self.shape.scale
    }
}

#[godot_api]
impl IAbstractShape for Circle {
    fn area(&self) -> f64 {
        std::f64::consts::PI * self.radius * self.radius
    }

    fn scaled_name(&mut self, factor: i64) -> GString {
        self.radius *= factor as f64;
        GString::from(&format!("circle*{factor}"))
    }
}

#[derive(GodotClass)]
#[class(init, base = AbstractShape)]
pub struct Square {
    shape: AbstractShape,

    #[var]
    side: f64,
}

#[godot_api]
impl Square {}

#[godot_api]
impl IAbstractShape for Square {
    fn area(&self) -> f64 {
        self.side * self.side
    }

    fn scaled_name(&mut self, factor: i64) -> GString {
        self.side *= factor as f64;
        GString::from(&format!("square*{factor}"))
    }
}

// Intermediate abstract class, to test multi-level Rust inheritance.
#[derive(GodotClass)]
#[class(abstract, base = Node)]
pub struct AbstractActor {
    base: Base<Node>,

    #[var]
    #[init(val = 10)]
    health: i64,
}

#[godot_api]
impl AbstractActor {
    #[func(abstract)]
    fn actor_name(&self) -> GString;

    #[func]
    fn take_damage(&mut self, amount: i64) -> i64 {
        self.health -= amount;
        self.health
    }
}

#[derive(GodotClass)]
#[class(abstract, base = AbstractActor)]
pub struct AbstractEnemy {
    actor: AbstractActor,

    #[var]
    #[init(val = 1)]
    threat: i64,
}

#[godot_api]
impl AbstractEnemy {
    #[func(abstract)]
    fn damage(&self) -> i64;

    #[func]
    fn threat_level(&self) -> i64 {
        self.threat * self.actor.health
    }
}

#[derive(GodotClass)]
#[class(init, base = AbstractEnemy)]
pub struct Goblin {
    enemy: AbstractEnemy,
}

#[godot_api]
impl Goblin {}

#[godot_api]
impl IAbstractActor for Goblin {
    fn actor_name(&self) -> GString {
        "goblin".into()
    }
}

#[godot_api]
impl IAbstractEnemy for Goblin {
    fn damage(&self) -> i64 {
        3
    }
}

#[godot_api]
impl INode for Goblin {
    fn ready(&mut self) {
        self.base_mut().set_name("goblin-ready");
    }
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Class-level metadata

#[itest]
fn abstract_class_registered_and_not_instantiable() {
    let class_db = ClassDb::singleton();

    assert!(class_db.class_exists("AbstractShape"));
    assert!(!class_db.can_instantiate("AbstractShape"));

    // Concrete subclasses can be instantiated.
    assert!(class_db.class_exists("Circle"));
    assert!(class_db.can_instantiate("Circle"));
}

#[itest]
fn abstract_class_is_godot_parent() {
    let class_db = ClassDb::singleton();

    assert_eq!(class_db.get_parent_class("Circle"), "AbstractShape");
    assert_eq!(class_db.get_parent_class("Square"), "AbstractShape");
    assert_eq!(class_db.get_parent_class("AbstractShape"), "Resource");

    assert!(class_db.is_parent_class("Circle", "AbstractShape"));
    assert!(class_db.is_parent_class("Circle", "Resource"));

    // Multi-level Rust hierarchy.
    assert_eq!(class_db.get_parent_class("Goblin"), "AbstractEnemy");
    assert_eq!(class_db.get_parent_class("AbstractEnemy"), "AbstractActor");
    assert!(class_db.is_parent_class("Goblin", "AbstractActor"));
    assert!(class_db.is_parent_class("Goblin", "Node"));
}

#[itest]
fn abstract_class_rust_inherits_relation() {
    // Rust-level `GodotClass::inherits()` must follow the Godot parent chain, not just `GodotClass::Base`.
    assert!(<Circle as GodotClass>::inherits::<AbstractShape>());
    assert!(<Circle as GodotClass>::inherits::<Resource>());
    assert!(<Goblin as GodotClass>::inherits::<AbstractEnemy>());
    assert!(<Goblin as GodotClass>::inherits::<AbstractActor>());
    assert!(<Goblin as GodotClass>::inherits::<Node>());
    assert!(!<Circle as GodotClass>::inherits::<AbstractActor>());
}

#[itest]
fn abstract_class_base_is_engine_class() {
    // `GodotClass::Base` must stay an engine class, so that `impl I<Engine>` and `Base<T>` keep working.
    let name = <Circle as GodotClass>::Base::class_id().to_string();
    assert_eq!(name, "Resource");

    let name = <Goblin as GodotClass>::Base::class_id().to_string();
    assert_eq!(name, "Node");
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Method-level metadata

#[itest]
fn abstract_func_registered_as_virtual_required() {
    let class_db = ClassDb::singleton();

    // Abstract methods are registered as virtual methods, which have no method bind -- so `class_has_method()` misses them. They are still
    // described in `class_get_method_list()`, see `abstract_func_metadata_flags()`.
    assert!(!class_db.class_has_method("AbstractShape", "area"));

    // The static function and the subclass implementations are regular methods.
    assert!(class_db.class_has_method("AbstractShape", "describe_kind"));
    assert!(class_db.class_has_method("Circle", "area"));
    assert!(class_db.class_has_method("Square", "area"));

    // Inherited lookup: `Circle::area` is found when querying without `no_inheritance`.
    assert!(class_db.class_has_method("Circle", "scaled_name"));
}

#[itest]
fn abstract_func_metadata_flags() {
    let class_db = ClassDb::singleton();

    // The abstract declaration itself goes through `classdb_register_extension_class_virtual_method()`, which ClassDB does not expose -- so
    // the abstract class's own method list must not contain it.
    let base_methods = class_db
        .class_get_method_list_ex("AbstractShape")
        .no_inheritance(true)
        .done();

    // The abstract declaration is visible as a virtual method: `class_has_method()` (method binds only) misses it, but the method list has it.
    let area_base = find_method(&base_methods, "area").expect("AbstractShape::area metadata");
    let flags = area_base.at("flags").to::<i64>();

    assert_ne!(flags & MethodFlags::VIRTUAL.ord() as i64, 0, "VIRTUAL");
    assert_ne!(flags & MethodFlags::CONST.ord() as i64, 0, "&self -> CONST");

    #[cfg(since_api = "4.5")]
    assert_ne!(
        flags & MethodFlags::VIRTUAL_REQUIRED.ord() as i64,
        0,
        "abstract methods must be flagged as required virtuals"
    );

    // The static function of the abstract class is a regular, non-virtual method.
    let describe = find_method(&base_methods, "describe_kind").expect("describe_kind");
    let flags = describe.at("flags").to::<i64>();
    assert_ne!(flags & MethodFlags::STATIC.ord() as i64, 0, "STATIC");
    assert_eq!(flags & MethodFlags::VIRTUAL.ord() as i64, 0, "!VIRTUAL");

    // The subclass's implementation is a regular, callable method -- not flagged virtual.
    let methods = class_db
        .class_get_method_list_ex("Circle")
        .no_inheritance(true)
        .done();

    let area = find_method(&methods, "area").expect("Circle::area must be registered");

    let flags = area.at("flags").to::<i64>();
    assert_eq!(flags & MethodFlags::VIRTUAL.ord() as i64, 0);
    assert_ne!(flags & MethodFlags::CONST.ord() as i64, 0, "&self -> CONST");

    // `scaled_name()` takes `&mut self`, so it must not be const.
    let scaled_name = find_method(&methods, "scaled_name").expect("Circle::scaled_name");
    let flags = scaled_name.at("flags").to::<i64>();
    assert_eq!(
        flags & MethodFlags::CONST.ord() as i64,
        0,
        "&mut self -> !CONST"
    );
}

fn find_method(method_list: &Array<VarDictionary>, name: &str) -> Option<VarDictionary> {
    method_list
        .iter_shared()
        .find(|dict| dict.at("name").to::<GString>() == name)
}

#[itest]
fn abstract_class_constant_inherited() {
    let class_db = ClassDb::singleton();
    assert!(class_db.class_has_integer_constant("AbstractShape", "SIDES_UNKNOWN"));
    assert_eq!(
        class_db.class_get_integer_constant("Circle", "SIDES_UNKNOWN"),
        -1
    );
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Behavior: polymorphism

#[itest]
fn abstract_class_polymorphic_call() {
    let mut circle = Circle::new_gd();
    circle.bind_mut().radius = 2.0;

    let mut square = Square::new_gd();
    square.bind_mut().side = 3.0;

    let shapes: Vec<Gd<AbstractShape>> = vec![circle.upcast(), square.upcast()];

    let areas: Vec<f64> = shapes
        .iter()
        .map(|shape| {
            shape
                .clone()
                .upcast::<Object>()
                .call("area", &[])
                .to::<f64>()
        })
        .collect();

    assert!((areas[0] - std::f64::consts::PI * 4.0).abs() < 1e-9);
    assert_eq!(areas[1], 9.0);
}

#[itest]
fn abstract_class_upcast_downcast() {
    let circle = Circle::new_gd();
    let shape: Gd<AbstractShape> = circle.clone().upcast();

    // Downcast back to the concrete class.
    let back = shape.clone().try_cast::<Circle>();
    assert!(back.is_ok());

    // Downcast to a sibling fails.
    assert!(shape.try_cast::<Square>().is_err());
}

#[itest]
fn abstract_class_bind_panics() {
    let circle = Circle::new_gd();
    let shape: Gd<AbstractShape> = circle.upcast();

    // An abstract class has no Rust instance of its own: a `Gd<AbstractShape>` always points at a concrete subclass object.
    expect_panic("bind() on abstract class", || {
        let _ = shape.bind();
    });
}

#[itest]
fn abstract_class_mutating_abstract_method() {
    let mut circle = Circle::new_gd();
    circle.bind_mut().radius = 2.0;

    let mut object = circle.clone().upcast::<Object>();
    let name = object
        .call("scaled_name", &[3.to_variant()])
        .to::<GString>();

    assert_eq!(name, "circle*3");
    assert_eq!(circle.bind().radius, 6.0);
}

#[itest]
fn abstract_class_static_func_inherited() {
    let mut object = Circle::new_gd().upcast::<Object>();
    // Statics of the abstract base are callable through subclass instances.
    let kind = object.call("describe_kind", &[]).to::<GString>();
    assert_eq!(kind, "shape");
}

#[itest]
fn abstract_class_subclass_own_methods() {
    let mut circle = Circle::new_gd();
    circle.bind_mut().radius = 1.0;

    let mut object = circle.upcast::<Object>();
    let circumference = object.call("circumference", &[]).to::<f64>();
    assert!((circumference - 2.0 * std::f64::consts::PI).abs() < 1e-9);
}

#[itest]
fn abstract_class_multilevel_calls() {
    let goblin = Goblin::new_alloc();

    {
        let mut object = goblin.clone().upcast::<Object>();
        assert_eq!(object.call("actor_name", &[]).to::<GString>(), "goblin");
        assert_eq!(object.call("damage", &[]).to::<i64>(), 3);
    }

    goblin.free();
}

#[itest]
fn abstract_class_engine_virtual_runs() {
    // `impl INode for Goblin` is possible because `GodotClass::Base` remains `Node`.
    let goblin = Goblin::new_alloc();
    let tree = godot::classes::Engine::singleton()
        .get_main_loop()
        .expect("main loop")
        .cast::<godot::classes::SceneTree>();

    let mut root = tree.get_root().expect("root viewport");
    root.add_child(&goblin);

    assert_eq!(goblin.get_name(), "goblin-ready");

    root.remove_child(&goblin);
    goblin.free();
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Base-class state and concrete base methods

#[itest]
fn base_class_state_inherited() {
    let circle = Circle::new_gd();

    // Base-class fields are reachable from Rust through the embedded base instance.
    assert_eq!(circle.bind().shape.scale, 1.0, "#[init(val)] on base field");

    // ...and as Godot properties of the *subclass*.
    let mut object = circle.clone().upcast::<Object>();
    object.set("scale", &2.5.to_variant());
    assert_eq!(circle.bind().shape.scale, 2.5);
    assert_eq!(object.get("scale").to::<f64>(), 2.5);

    object.set("label", &"ring".to_variant());
    assert_eq!(circle.bind().shape.label, "ring");
}

#[itest]
fn base_class_concrete_method_inherited() {
    let mut circle = Circle::new_gd();
    {
        let mut guard = circle.bind_mut();
        guard.shape.scale = 3.0;
        guard.shape.label = "ring".into();
    }

    let mut object = circle.clone().upcast::<Object>();

    // `&self` base method, operating on base state.
    assert_eq!(object.call("describe", &[]).to::<GString>(), "ring @3");

    // `&mut self` base method.
    assert_eq!(object.call("grow", &[0.5.to_variant()]).to::<f64>(), 3.5);
    assert_eq!(circle.bind().shape.scale, 3.5);

    // The base method sees the inherited `Base<T>` handle, whose dynamic class is the subclass.
    assert_eq!(
        object.call("base_class_name", &[]).to::<GString>(),
        "Circle"
    );
}

#[itest]
fn base_class_method_registered_on_subclass_only() {
    let class_db = ClassDb::singleton();

    // The abstract class's own methods and properties are registered for each concrete subclass, not for itself -- otherwise properties
    // would appear twice in the inspector, and the base's method binds would receive instances of the wrong type.
    assert!(class_db.class_has_method("Circle", "describe"));
    assert!(class_db.class_has_method("Square", "describe"));
    assert!(!class_db.class_has_method("AbstractShape", "describe"));

    let own_props = class_db
        .class_get_property_list_ex("Circle")
        .no_inheritance(true)
        .done();
    let scale_count = own_props
        .iter_shared()
        .filter(|dict| dict.at("name").to::<GString>() == "scale")
        .count();
    assert_eq!(scale_count, 1, "inherited property registered exactly once");
}

#[itest]
fn base_class_state_is_per_instance() {
    let circle = Circle::new_gd();
    let square = Square::new_gd();

    circle
        .clone()
        .upcast::<Object>()
        .set("scale", &2.0.to_variant());

    assert_eq!(circle.bind().shape.scale, 2.0);
    assert_eq!(square.bind().shape.scale, 1.0, "siblings are independent");
}

#[itest]
fn base_class_reentrant_borrow() {
    // The projection borrows through the *subclass*'s GdCell, so a base method called from a subclass method must not alias.
    let mut circle = Circle::new_gd();
    circle.bind_mut().radius = 2.0;

    let mut object = circle.clone().upcast::<Object>();
    let area = object.call("scaled_area", &[]).to::<f64>();
    assert!((area - std::f64::consts::PI * 4.0).abs() < 1e-9);
}

#[itest]
fn base_class_multilevel_state_and_methods() {
    let goblin = Goblin::new_alloc();

    {
        let guard = goblin.bind();
        assert_eq!(guard.enemy.threat, 1);
        assert_eq!(guard.enemy.actor.health, 10);
    }

    let mut object = goblin.clone().upcast::<Object>();

    // Method from the direct base...
    assert_eq!(object.call("threat_level", &[]).to::<i64>(), 10);

    // ...and from the grandparent.
    assert_eq!(object.call("take_damage", &[4.to_variant()]).to::<i64>(), 6);
    assert_eq!(goblin.bind().enemy.actor.health, 6);

    // Properties from both levels.
    assert_eq!(object.get("health").to::<i64>(), 6);
    assert_eq!(object.get("threat").to::<i64>(), 1);

    goblin.free();
}

#[itest]
fn base_class_multilevel_property_registration() {
    let class_db = ClassDb::singleton();

    // Both ancestors' members land on the concrete class, once each.
    assert!(class_db.class_has_method("Goblin", "take_damage"));
    assert!(class_db.class_has_method("Goblin", "threat_level"));
    assert!(!class_db.class_has_method("AbstractActor", "take_damage"));

    let own_props = class_db
        .class_get_property_list_ex("Goblin")
        .no_inheritance(true)
        .done();

    for name in ["health", "threat"] {
        let count = own_props
            .iter_shared()
            .filter(|dict| dict.at("name").to::<GString>() == name)
            .count();
        assert_eq!(count, 1, "property `{name}` registered exactly once");
    }
}

#[itest]
fn abstract_class_variant_roundtrip() {
    let circle = Circle::new_gd();
    let shape: Gd<AbstractShape> = circle.upcast();

    let variant = shape.to_variant();
    let back = variant.to::<Gd<AbstractShape>>();
    assert_eq!(back.instance_id(), shape.instance_id());

    let as_array = varray![&shape];
    assert_eq!(as_array.len(), 1);
}
