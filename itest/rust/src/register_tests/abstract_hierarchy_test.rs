/*
 * Copyright (c) godot-rust; Bromeon and contributors.
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

//! Hierarchy shapes and edge cases for `#[class(abstract)]` / `#[class(base = SomeRustClass)]`.
//!
//! `abstract_class_test.rs` covers the straightforward cases; this file exercises broad and deep hierarchies, partial implementations,
//! different engine bases, inherited signals and properties with custom accessors, and the surrounding APIs.

use godot::builtin::{Array, VarDictionary};
use godot::classes::{ClassDb, Node, Object, RefCounted, Resource};
use godot::obj::{EngineBitfield, NewAlloc};
use godot::prelude::*;

use crate::framework::itest;

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Hierarchy 1: broad -- one abstract base (default `RefCounted`), four concrete siblings.

#[derive(GodotClass)]
#[class(abstract)]
pub struct BroadBase {
    base: Base<RefCounted>,

    #[var]
    #[init(val = 1)]
    weight: i64,
}

#[godot_api]
impl BroadBase {
    #[func(abstract)]
    fn letter(&self) -> GString;

    /// Concrete base method, inherited by all four siblings.
    #[func]
    fn weighted_letter(&self) -> GString {
        GString::from(&format!("{}x{}", self.letter(), self.weight))
    }
}

macro_rules! broad_sibling {
    ($class:ident, $letter:literal) => {
        #[derive(GodotClass)]
        #[class(init, base = BroadBase)]
        pub struct $class {
            broad: BroadBase,
        }

        #[godot_api]
        impl $class {}

        #[godot_api]
        impl IBroadBase for $class {
            fn letter(&self) -> GString {
                $letter.into()
            }
        }
    };
}

broad_sibling!(BroadA, "a");
broad_sibling!(BroadB, "b");
broad_sibling!(BroadC, "c");
broad_sibling!(BroadD, "d");

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Hierarchy 2: deep -- four levels, with a *partial* implementation in the middle.
//
// `DeepL1` declares two abstract methods. `DeepL2` satisfies one of them with an ordinary `#[func]` of the same name (inherited by every
// descendant), and adds an abstract method of its own. The concrete leaf only has to supply what is still missing.

#[derive(GodotClass)]
#[class(abstract, base = Node)]
pub struct DeepL1 {
    base: Base<Node>,

    #[var]
    #[init(val = 1)]
    l1: i64,
}

#[godot_api]
impl DeepL1 {
    #[func(abstract)]
    fn provided_by_l2(&self) -> i64;

    #[func(abstract)]
    fn provided_by_leaf(&self) -> i64;

    #[func]
    fn sum_l1(&self) -> i64 {
        self.l1 + self.provided_by_l2() + self.provided_by_leaf()
    }
}

#[derive(GodotClass)]
#[class(abstract, base = DeepL1)]
pub struct DeepL2 {
    l1: DeepL1,

    #[var]
    #[init(val = 2)]
    l2: i64,
}

#[godot_api]
impl DeepL2 {
    /// Satisfies `DeepL1::provided_by_l2` for all descendants: a plain method registered for each concrete subclass.
    #[func]
    fn provided_by_l2(&self) -> i64 {
        self.l2 * 10
    }

    #[func(abstract)]
    fn provided_by_l3(&self) -> GString;
}

#[derive(GodotClass)]
#[class(abstract, base = DeepL2)]
pub struct DeepL3 {
    l2: DeepL2,

    #[var]
    #[init(val = 3)]
    l3: i64,
}

#[godot_api]
impl DeepL3 {
    #[func]
    fn provided_by_l3(&self) -> GString {
        GString::from(&format!("l3={}", self.l3))
    }

    #[func]
    fn depth(&self) -> i64 {
        3
    }
}

#[derive(GodotClass)]
#[class(init, base = DeepL3)]
pub struct DeepLeaf {
    l3: DeepL3,

    #[var]
    #[init(val = 4)]
    leaf: i64,
}

#[godot_api]
impl DeepLeaf {
    #[func]
    fn provided_by_leaf(&self) -> i64 {
        self.leaf * 100
    }
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Hierarchy 3: abstract class with no abstract methods at all -- pure shared state and behavior, so no `I<Class>` trait is generated.

#[derive(GodotClass)]
#[class(abstract, base = Resource)]
pub struct StateOnlyBase {
    base: Base<Resource>,

    #[var]
    counter: i64,
}

#[godot_api]
impl StateOnlyBase {
    #[func]
    fn bump(&mut self) -> i64 {
        self.counter += 1;
        self.counter
    }
}

#[derive(GodotClass)]
#[class(init, base = StateOnlyBase)]
pub struct StateOnlyUser {
    state: StateOnlyBase,
}

#[godot_api]
impl StateOnlyUser {}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Hierarchy 4: signals, custom accessors and exports declared on the abstract base.

#[derive(GodotClass)]
#[class(abstract, base = Resource)]
pub struct SignalBase {
    base: Base<Resource>,

    #[var(get = get_doubled, set = set_doubled)]
    #[init(val = 0)]
    doubled: i64,

    #[export(range = (0.0, 10.0))]
    #[init(val = 5.0)]
    ranged: f64,
}

#[godot_api]
impl SignalBase {
    #[signal]
    fn base_changed(value: i64);

    #[func]
    fn get_doubled(&self) -> i64 {
        self.doubled * 2
    }

    #[func]
    fn set_doubled(&mut self, value: i64) {
        self.doubled = value;
        self.signals().base_changed().emit(value);
    }
}

#[derive(GodotClass)]
#[class(init, base = SignalBase)]
pub struct SignalUser {
    signal_base: SignalBase,
}

#[godot_api]
impl SignalUser {}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Hierarchy 5: manually-managed base (`Object`), to check memory management is taken from the engine base.

#[derive(GodotClass)]
#[class(abstract, base = Object)]
pub struct ManualBase {
    base: Base<Object>,

    #[var]
    #[init(val = 42)]
    tag: i64,
}

#[godot_api]
impl ManualBase {
    #[func]
    fn tag_twice(&self) -> i64 {
        self.tag * 2
    }
}

#[derive(GodotClass)]
#[class(init, base = ManualBase)]
pub struct ManualUser {
    manual: ManualBase,
}

#[godot_api]
impl ManualUser {}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Consumers: abstract classes in `#[func]` signatures and containers.

#[derive(GodotClass)]
#[class(init, base = RefCounted)]
pub struct AbstractConsumer {
    base: Base<RefCounted>,
}

#[godot_api]
impl AbstractConsumer {
    /// Accepts any subclass of the abstract class, polymorphically.
    #[func]
    fn total_weight(&self, shapes: Array<Gd<BroadBase>>) -> i64 {
        shapes
            .iter_shared()
            .map(|shape| {
                shape
                    .upcast::<Object>()
                    .call("weighted_letter", &[])
                    .to::<GString>()
                    .len() as i64
            })
            .sum()
    }

    #[func]
    fn pick_first(&self, shapes: Array<Gd<BroadBase>>) -> Option<Gd<BroadBase>> {
        shapes.iter_shared().next()
    }
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Tests: broad hierarchy

#[itest]
fn broad_all_siblings_registered() {
    let class_db = ClassDb::singleton();

    for name in ["BroadA", "BroadB", "BroadC", "BroadD"] {
        assert!(class_db.class_exists(name), "{name} exists");
        assert!(class_db.can_instantiate(name), "{name} instantiable");
        assert_eq!(class_db.get_parent_class(name), "BroadBase");

        // Inherited concrete method and property land on every sibling.
        assert!(class_db.class_has_method(name, "weighted_letter"));
        assert_eq!(count_own_property(name, "weight"), 1, "{name}.weight");
    }

    assert!(!class_db.can_instantiate("BroadBase"));
    assert!(!class_db.class_has_method("BroadBase", "weighted_letter"));
}

#[itest]
fn broad_polymorphic_dispatch() {
    let siblings: Vec<Gd<BroadBase>> = vec![
        BroadA::new_gd().upcast(),
        BroadB::new_gd().upcast(),
        BroadC::new_gd().upcast(),
        BroadD::new_gd().upcast(),
    ];

    let letters: Vec<String> = siblings
        .iter()
        .map(|s| {
            s.clone()
                .upcast::<Object>()
                .call("weighted_letter", &[])
                .to::<GString>()
                .to_string()
        })
        .collect();

    assert_eq!(letters, vec!["ax1", "bx1", "cx1", "dx1"]);
}

#[itest]
fn broad_sibling_state_independent() {
    let a = BroadA::new_gd();
    let b = BroadB::new_gd();

    a.clone().upcast::<Object>().set("weight", &7.to_variant());

    assert_eq!(a.bind().broad.weight, 7);
    assert_eq!(b.bind().broad.weight, 1);

    // The inherited concrete method sees the updated base state.
    let letter = a
        .upcast::<Object>()
        .call("weighted_letter", &[])
        .to::<GString>();
    assert_eq!(letter, "ax7");
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Tests: deep hierarchy with partial implementation

#[itest]
fn deep_chain_parent_classes() {
    let class_db = ClassDb::singleton();

    assert_eq!(class_db.get_parent_class("DeepLeaf"), "DeepL3");
    assert_eq!(class_db.get_parent_class("DeepL3"), "DeepL2");
    assert_eq!(class_db.get_parent_class("DeepL2"), "DeepL1");
    assert_eq!(class_db.get_parent_class("DeepL1"), "Node");

    assert!(class_db.is_parent_class("DeepLeaf", "DeepL1"));
    assert!(class_db.is_parent_class("DeepLeaf", "Node"));

    for abstract_name in ["DeepL1", "DeepL2", "DeepL3"] {
        assert!(
            !class_db.can_instantiate(abstract_name),
            "{abstract_name} must be abstract"
        );
    }
    assert!(class_db.can_instantiate("DeepLeaf"));
}

#[itest]
fn deep_chain_partial_implementation() {
    let leaf = DeepLeaf::new_alloc();
    let mut object = leaf.clone().upcast::<Object>();

    // `provided_by_l2` comes from the middle of the chain, `provided_by_leaf` from the leaf itself.
    assert_eq!(object.call("provided_by_l2", &[]).to::<i64>(), 20);
    assert_eq!(object.call("provided_by_leaf", &[]).to::<i64>(), 400);
    assert_eq!(object.call("provided_by_l3", &[]).to::<GString>(), "l3=3");

    // A base method calling abstract methods dispatches to whatever supplied them.
    assert_eq!(object.call("sum_l1", &[]).to::<i64>(), 1 + 20 + 400);

    leaf.free();
}

#[itest]
fn deep_chain_all_levels_state() {
    let leaf = DeepLeaf::new_alloc();

    {
        let guard = leaf.bind();
        assert_eq!(guard.leaf, 4);
        assert_eq!(guard.l3.l3, 3);
        assert_eq!(guard.l3.l2.l2, 2);
        assert_eq!(guard.l3.l2.l1.l1, 1);
    }

    let mut object = leaf.clone().upcast::<Object>();
    for (name, expected) in [("l1", 1), ("l2", 2), ("l3", 3), ("leaf", 4)] {
        assert_eq!(object.get(name).to::<i64>(), expected, "property {name}");
        assert_eq!(count_own_property("DeepLeaf", name), 1, "property {name}");
    }

    object.set("l1", &11.to_variant());
    assert_eq!(leaf.bind().l3.l2.l1.l1, 11);

    leaf.free();
}

#[itest]
fn deep_chain_methods_registered_once() {
    let class_db = ClassDb::singleton();

    // Every ancestor's concrete methods land on the leaf...
    for name in [
        "sum_l1",
        "provided_by_l2",
        "provided_by_l3",
        "depth",
        "provided_by_leaf",
    ] {
        assert!(class_db.class_has_method("DeepLeaf", name), "{name}");
    }

    // ...and on no intermediate abstract class.
    assert!(!class_db.class_has_method("DeepL1", "sum_l1"));
    assert!(!class_db.class_has_method("DeepL2", "provided_by_l2"));
    assert!(!class_db.class_has_method("DeepL3", "depth"));
}

#[itest]
fn deep_chain_base_field_through_chain() {
    let mut leaf = DeepLeaf::new_alloc();

    // `Base<Node>` lives in `DeepL1` and is reached through three levels of embedding.
    leaf.bind_mut().base_mut().set_name("deep-leaf");
    assert_eq!(leaf.get_name(), "deep-leaf");

    // `to_gd()` from the deepest base returns the concrete object.
    let via_base = leaf.bind().l3.l2.l1.to_gd();
    assert_eq!(via_base.instance_id(), leaf.instance_id());

    leaf.free();
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Tests: abstract class without abstract methods

#[itest]
fn state_only_base_without_abstract_methods() {
    let class_db = ClassDb::singleton();

    assert!(!class_db.can_instantiate("StateOnlyBase"));
    assert!(class_db.can_instantiate("StateOnlyUser"));
    assert!(class_db.class_has_method("StateOnlyUser", "bump"));
    assert_eq!(count_own_property("StateOnlyUser", "counter"), 1);

    let user = StateOnlyUser::new_gd();
    let mut object = user.clone().upcast::<Object>();

    assert_eq!(object.call("bump", &[]).to::<i64>(), 1);
    assert_eq!(object.call("bump", &[]).to::<i64>(), 2);
    assert_eq!(user.bind().state.counter, 2);
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Tests: signals, custom accessors and exports on the base

#[itest]
fn base_signal_inherited() {
    let class_db = ClassDb::singleton();

    // Signals stay on the abstract class and are inherited through ClassDB, so they are not duplicated.
    assert!(class_db.class_has_signal("SignalBase", "base_changed"));
    assert!(class_db.class_has_signal("SignalUser", "base_changed"));

    let user = SignalUser::new_gd();
    let mut object = user.clone().upcast::<Object>();

    // Emitted from an inherited setter; connect through the untyped API, which is what GDScript would use.
    let received = Array::<i64>::new();
    let received_clone = received.clone();
    let callable = Callable::from_fn("on_changed", move |args: &[&Variant]| {
        received_clone.clone().push(args[0].to::<i64>());
        Variant::nil()
    });
    object.connect("base_changed", &callable);

    object.set("doubled", &21.to_variant());

    assert_eq!(received.len(), 1);
    assert_eq!(received.at(0), 21);
}

#[itest]
fn base_custom_accessors_inherited() {
    let user = SignalUser::new_gd();
    let mut object = user.clone().upcast::<Object>();

    // Custom getter/setter are ordinary `#[func]`s of the base, registered for the subclass.
    object.set("doubled", &5.to_variant());
    assert_eq!(user.bind().signal_base.doubled, 5);
    assert_eq!(object.get("doubled").to::<i64>(), 10, "getter doubles");

    assert!(ClassDb::singleton().class_has_method("SignalUser", "get_doubled"));
    assert!(ClassDb::singleton().class_has_method("SignalUser", "set_doubled"));
}

#[itest]
fn base_export_hint_inherited() {
    let props = ClassDb::singleton()
        .class_get_property_list_ex("SignalUser")
        .no_inheritance(true)
        .done();

    let ranged = props
        .iter_shared()
        .find(|dict| dict.at("name").to::<GString>() == "ranged")
        .expect("inherited export");

    // `#[export(range = ...)]` keeps its hint string when registered for the subclass.
    assert_eq!(ranged.at("hint_string").to::<GString>(), "0,10");
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Tests: manually-managed base

#[itest]
fn manual_memory_base() {
    let class_db = ClassDb::singleton();
    assert_eq!(class_db.get_parent_class("ManualUser"), "ManualBase");
    assert_eq!(class_db.get_parent_class("ManualBase"), "Object");

    let user = ManualUser::new_alloc();
    let mut object = user.clone().upcast::<Object>();

    assert_eq!(object.call("tag_twice", &[]).to::<i64>(), 84);
    assert_eq!(object.get("tag").to::<i64>(), 42);

    user.free();
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Tests: abstract classes in signatures and containers

#[itest]
fn abstract_class_in_typed_array() {
    let mut array = Array::<Gd<BroadBase>>::new();
    array.push(&BroadA::new_gd().upcast::<BroadBase>());
    array.push(&BroadB::new_gd().upcast::<BroadBase>());

    let consumer = AbstractConsumer::new_gd();
    let mut object = consumer.upcast::<Object>();

    // "ax1" and "bx1" are 3 chars each.
    let total = object
        .call("total_weight", &[array.to_variant()])
        .to::<i64>();
    assert_eq!(total, 6);

    let first = object
        .call("pick_first", &[array.to_variant()])
        .to::<Gd<BroadBase>>();
    assert!(first.try_cast::<BroadA>().is_ok());
}

#[itest]
fn abstract_class_instantiate_refused() {
    let class_db = ClassDb::singleton();

    // Godot must not hand out instances of an abstract class.
    assert!(!class_db.can_instantiate("BroadBase"));
    assert!(!class_db.can_instantiate("DeepL2"));
    assert_eq!(class_db.instantiate("BroadBase"), Variant::nil());
}

#[itest]
fn abstract_class_rust_inherits_deep() {
    assert!(<DeepLeaf as GodotClass>::inherits::<DeepL3>());
    assert!(<DeepLeaf as GodotClass>::inherits::<DeepL2>());
    assert!(<DeepLeaf as GodotClass>::inherits::<DeepL1>());
    assert!(<DeepLeaf as GodotClass>::inherits::<Node>());
    assert!(!<DeepLeaf as GodotClass>::inherits::<BroadBase>());

    // Upcasts to every ancestor compile and preserve identity.
    let leaf = DeepLeaf::new_alloc();
    let id = leaf.instance_id();

    assert_eq!(leaf.clone().upcast::<DeepL3>().instance_id(), id);
    assert_eq!(leaf.clone().upcast::<DeepL2>().instance_id(), id);
    assert_eq!(leaf.clone().upcast::<DeepL1>().instance_id(), id);
    assert_eq!(leaf.clone().upcast::<Node>().instance_id(), id);

    // Downcast from an intermediate abstract class to a sibling-free leaf.
    let as_l2: Gd<DeepL2> = leaf.clone().upcast();
    assert!(as_l2.try_cast::<DeepLeaf>().is_ok());

    leaf.free();
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Helpers

/// Number of times `property` appears in `class`'s *own* property list (i.e. excluding inherited ones).
fn count_own_property(class: &str, property: &str) -> usize {
    let props: Array<VarDictionary> = ClassDb::singleton()
        .class_get_property_list_ex(class)
        .no_inheritance(true)
        .done();

    props
        .iter_shared()
        .filter(|dict| dict.at("name").to::<GString>() == property)
        .count()
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Hierarchy 6: `&mut self` abstract method invoked from a concrete base method (re-entrant borrow through Godot).

#[derive(GodotClass)]
#[class(abstract, base = RefCounted)]
pub struct MutBase {
    base: Base<RefCounted>,

    #[var]
    log: GString,
}

#[godot_api]
impl MutBase {
    /// `&mut self` abstract method: callable from outside, but not from the base's own code -- re-entering the object from Rust needs a
    /// shared outer borrow, so no forwarder is generated for it.
    #[func(abstract)]
    fn mutate(&mut self, amount: i64) -> i64;

    /// `&self` abstract method, which the base *can* dispatch to.
    #[func(abstract)]
    fn peek(&self) -> i64;

    /// Template method: a concrete base method reading through a `&self` abstract hook, twice.
    #[func]
    fn peek_sum(&self) -> i64 {
        self.peek() + self.peek()
    }
}

#[derive(GodotClass)]
#[class(init, base = MutBase)]
pub struct MutUser {
    mut_base: MutBase,

    #[var]
    total: i64,
}

#[godot_api]
impl MutUser {}

#[godot_api]
impl IMutBase for MutUser {
    fn mutate(&mut self, amount: i64) -> i64 {
        self.total += amount;
        self.total
    }

    fn peek(&self) -> i64 {
        self.total
    }
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Hierarchy 7: `OnReady` fields and a script-virtual `#[func(virtual)]` declared on the abstract base.

#[derive(GodotClass)]
#[class(abstract, base = Node)]
pub struct ReadyBase {
    base: Base<Node>,

    #[init(val = OnReady::manual())]
    late: OnReady<i64>,

    #[var]
    ready_ran: bool,
}

#[godot_api]
impl ReadyBase {
    #[func]
    fn late_value(&self) -> i64 {
        *self.late
    }
}

#[derive(GodotClass)]
#[class(init, base = ReadyBase)]
pub struct ReadyUser {
    ready: ReadyBase,
}

#[godot_api]
impl ReadyUser {}

#[godot_api]
impl INode for ReadyUser {
    fn ready(&mut self) {
        // The base's `__before_ready()` has already run at this point.
        self.ready.late.init(99);
        self.ready.ready_ran = true;
    }
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Hierarchy 8: constants at several levels, and an abstract class used as an exported property type.

#[derive(GodotClass)]
#[class(abstract, base = Resource)]
pub struct ConstBase {
    base: Base<Resource>,
}

#[godot_api]
impl ConstBase {
    #[constant]
    const LEVEL: i64 = 1;

    #[func(abstract)]
    fn id(&self) -> i64;
}

#[derive(GodotClass)]
#[class(abstract, base = ConstBase)]
pub struct ConstMid {
    const_base: ConstBase,
}

#[godot_api]
impl ConstMid {
    #[constant]
    const MID_LEVEL: i64 = 2;
}

#[derive(GodotClass)]
#[class(init, base = ConstMid)]
pub struct ConstLeaf {
    const_mid: ConstMid,
}

#[godot_api]
impl ConstLeaf {
    #[constant]
    const LEAF_LEVEL: i64 = 3;
}

#[godot_api]
impl IConstBase for ConstLeaf {
    fn id(&self) -> i64 {
        7
    }
}

#[derive(GodotClass)]
#[class(init, base = Resource)]
pub struct HoldsAbstract {
    base: Base<Resource>,

    /// An abstract class is a perfectly good exported property type.
    #[export]
    shape: Option<Gd<ConstBase>>,
}

#[godot_api]
impl HoldsAbstract {}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Tests: re-entrant `&mut self` dispatch

#[itest]
fn base_dispatches_to_shared_abstract_method() {
    let mut user = MutUser::new_gd();
    user.bind_mut().total = 4;

    let mut object = user.clone().upcast::<Object>();

    // `peek_sum` is a base method; each `self.peek()` re-enters the subclass through Godot. Both borrows are shared, so this is allowed.
    assert_eq!(object.call("peek_sum", &[]).to::<i64>(), 8);

    // Same thing entered from Rust rather than from Godot.
    assert_eq!(user.bind().mut_base.peek_sum(), 8);
}

#[itest]
fn mut_abstract_method_callable_from_outside() {
    let user = MutUser::new_gd();
    let mut object = user.clone().upcast::<Object>();

    // A `&mut self` abstract method works normally when called on the object; only the base class's own code cannot dispatch to it.
    assert_eq!(object.call("mutate", &[5.to_variant()]).to::<i64>(), 5);
    assert_eq!(object.call("mutate", &[2.to_variant()]).to::<i64>(), 7);
    assert_eq!(user.bind().total, 7);
}

#[itest]
fn base_mut_rejected_on_abstract_class() {
    // `base_mut()` needs this instance's storage, which for an abstract class holds a subclass's struct. Rejected rather than reinterpreted.
    let mut user = MutUser::new_gd();

    crate::framework::expect_panic("base_mut() on abstract class", move || {
        let mut guard = user.bind_mut();
        let _ = godot::obj::WithBaseField::base_mut(&mut guard.mut_base);
    });
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Tests: OnReady and script-virtual on the base

#[itest]
fn base_onready_initialized_via_chain() {
    let user = ReadyUser::new_alloc();
    let mut root = scene_tree_root();
    root.add_child(&user);

    {
        let guard = user.bind();
        assert!(guard.ready.ready_ran, "subclass ready() ran");
        assert_eq!(*guard.ready.late, 99, "base OnReady field usable");
    }

    // The base's concrete method reads the base's own `OnReady` field.
    let mut object = user.clone().upcast::<Object>();
    assert_eq!(object.call("late_value", &[]).to::<i64>(), 99);

    root.remove_child(&user);
    user.free();
}

/// `#[func(virtual)]` needs Godot 4.3; the proc-macro rejects it regardless of `#[cfg]`, so the whole hierarchy is gated.
#[cfg(since_api = "4.3")]
mod script_virtual {
    use super::*;

    #[derive(GodotClass)]
    #[class(abstract, base = RefCounted)]
    pub struct VirtualBase {
        base: Base<RefCounted>,
    }

    #[godot_api]
    impl VirtualBase {
        /// Script-overridable method declared on the base, inherited by subclasses.
        #[func(virtual)]
        fn flavor(&self) -> GString {
            "rust-base".into()
        }
    }

    #[derive(GodotClass)]
    #[class(init, base = VirtualBase)]
    pub struct VirtualUser {
        virtual_base: VirtualBase,
    }

    #[godot_api]
    impl VirtualUser {}

    #[itest]
    fn base_script_virtual_inherited() {
        // `#[func(virtual)]` has no callable method bind -- it is virtual metadata, telling scripts they may override `_flavor`.
        let class_db = ClassDb::singleton();
        assert!(!class_db.class_has_method("VirtualUser", "_flavor"));

        let own_methods = class_db
            .class_get_method_list_ex("VirtualUser")
            .no_inheritance(true)
            .done();
        let flavor = own_methods
            .iter_shared()
            .find(|dict| dict.at("name").to::<GString>() == "_flavor")
            .expect("inherited script-virtual metadata on the subclass");
        assert_ne!(
            flavor.at("flags").to::<i64>()
                & godot::register::info::MethodFlags::VIRTUAL.ord() as i64,
            0
        );

        // Not on the abstract class, which has no instances to dispatch on.
        let base_methods = class_db
            .class_get_method_list_ex("VirtualBase")
            .no_inheritance(true)
            .done();
        assert!(
            !base_methods
                .iter_shared()
                .any(|dict| dict.at("name").to::<GString>() == "_flavor")
        );

        // The Rust-side dispatcher falls back to the base's default when no script overrides it.
        let user = VirtualUser::new_gd();
        assert_eq!(user.bind().virtual_base.flavor(), "rust-base");
    }
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Tests: constants and exported abstract types

#[itest]
fn constants_at_every_level() {
    let class_db = ClassDb::singleton();

    // Constants stay on their declaring class and are inherited through ClassDB.
    assert_eq!(class_db.class_get_integer_constant("ConstLeaf", "LEVEL"), 1);
    assert_eq!(
        class_db.class_get_integer_constant("ConstLeaf", "MID_LEVEL"),
        2
    );
    assert_eq!(
        class_db.class_get_integer_constant("ConstLeaf", "LEAF_LEVEL"),
        3
    );

    assert!(class_db.class_has_integer_constant("ConstBase", "LEVEL"));
    assert!(class_db.class_has_integer_constant("ConstMid", "MID_LEVEL"));
}

#[itest]
fn abstract_class_as_exported_property() {
    let props = ClassDb::singleton()
        .class_get_property_list_ex("HoldsAbstract")
        .no_inheritance(true)
        .done();

    let shape = props
        .iter_shared()
        .find(|dict| dict.at("name").to::<GString>() == "shape")
        .expect("exported abstract-typed property");

    // The editor hint names the abstract class, so the inspector offers its subclasses.
    assert_eq!(shape.at("hint_string").to::<GString>(), "ConstBase");

    let mut holder = HoldsAbstract::new_gd();
    let leaf = ConstLeaf::new_gd();
    holder.bind_mut().shape = Some(leaf.clone().upcast());

    let object = holder.clone().upcast::<Object>();
    let read_back = object.get("shape").to::<Gd<ConstBase>>();
    assert_eq!(read_back.instance_id(), leaf.instance_id());

    // Dispatch through the abstract-typed handle.
    assert_eq!(read_back.upcast::<Object>().call("id", &[]).to::<i64>(), 7);
}

#[itest]
fn abstract_intermediate_adds_nothing() {
    let class_db = ClassDb::singleton();

    // `ConstMid` declares no members of its own; the chain must still be intact.
    assert_eq!(class_db.get_parent_class("ConstLeaf"), "ConstMid");
    assert_eq!(class_db.get_parent_class("ConstMid"), "ConstBase");
    assert!(class_db.class_has_method("ConstLeaf", "id"));
    assert!(!class_db.can_instantiate("ConstMid"));
}

// ----------------------------------------------------------------------------------------------------------------------------------------------
// Tests: documented limitation -- engine virtuals of an abstract base are not inherited

#[itest]
fn engine_virtual_on_base_not_inherited() {
    // Godot asks the most-derived extension class for its virtual table, so `ReadyUser::ready()` is what runs; `ReadyBase` could not
    // contribute one. This test pins the documented behavior.
    let user = ReadyUser::new_alloc();
    let mut root = scene_tree_root();
    root.add_child(&user);

    assert!(user.bind().ready.ready_ran);

    root.remove_child(&user);
    user.free();
}

fn scene_tree_root() -> Gd<godot::classes::Window> {
    godot::classes::Engine::singleton()
        .get_main_loop()
        .expect("main loop")
        .cast::<godot::classes::SceneTree>()
        .get_root()
        .expect("root viewport")
}
