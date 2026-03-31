//! Tests for the deps command

use super::analysis::{check_architecture, detect_cycles};
use super::graph::build_dependents_map;
use crate::dag::{Dag, DagNode};

#[test]
fn test_build_dependents_map() {
    let mut dag = Dag::new();

    dag.add_node(DagNode {
        id: "apps/web".to_string(),
        name: "web".to_string(),
        path: "apps/web".to_string(),
        deps: vec!["libs/ui".to_string()],
    });

    dag.add_node(DagNode {
        id: "libs/ui".to_string(),
        name: "ui".to_string(),
        path: "libs/ui".to_string(),
        deps: vec![],
    });

    let dependents = build_dependents_map(&dag);

    assert!(
        dependents
            .get("libs/ui")
            .unwrap()
            .contains(&"apps/web".to_string())
    );
    assert!(dependents.get("apps/web").unwrap().is_empty());
}

#[test]
fn test_detect_cycles_no_cycle() {
    let mut dag = Dag::new();

    dag.add_node(DagNode {
        id: "a".to_string(),
        name: "a".to_string(),
        path: "apps/a".to_string(),
        deps: vec!["b".to_string()],
    });

    dag.add_node(DagNode {
        id: "b".to_string(),
        name: "b".to_string(),
        path: "libs/b".to_string(),
        deps: vec![],
    });

    let cycles = detect_cycles(&dag);
    assert!(cycles.is_empty());
}

#[test]
fn test_detect_cycles_with_cycle() {
    let mut dag = Dag::new();

    dag.add_node(DagNode {
        id: "a".to_string(),
        name: "a".to_string(),
        path: "libs/a".to_string(),
        deps: vec!["b".to_string()],
    });

    dag.add_node(DagNode {
        id: "b".to_string(),
        name: "b".to_string(),
        path: "libs/b".to_string(),
        deps: vec!["a".to_string()],
    });

    let cycles = detect_cycles(&dag);
    assert!(!cycles.is_empty());
}

#[test]
fn test_check_architecture_violation() {
    let mut dag = Dag::new();

    dag.add_node(DagNode {
        id: "apps/web".to_string(),
        name: "web".to_string(),
        path: "apps/web".to_string(),
        deps: vec!["apps/api".to_string()],
    });

    dag.add_node(DagNode {
        id: "apps/api".to_string(),
        name: "api".to_string(),
        path: "apps/api".to_string(),
        deps: vec![],
    });

    let violations = check_architecture(&dag);
    assert!(!violations.is_empty());
    assert!(violations[0].contains("Cross-app dependency"));
}

#[test]
fn test_check_architecture_valid() {
    let mut dag = Dag::new();

    dag.add_node(DagNode {
        id: "apps/web".to_string(),
        name: "web".to_string(),
        path: "apps/web".to_string(),
        deps: vec!["libs/ui".to_string()],
    });

    dag.add_node(DagNode {
        id: "libs/ui".to_string(),
        name: "ui".to_string(),
        path: "libs/ui".to_string(),
        deps: vec![],
    });

    let violations = check_architecture(&dag);
    assert!(violations.is_empty());
}
