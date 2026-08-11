use super::*;

#[test]
fn the_catalog_agrees_with_the_json() {
    for machine in MACHINES {
        let data = GameData::load_machine(machine).unwrap();
        println!("{:<11} ante {:?}", machine.id, data.ante().is_some());
    }
    let dragon = GameData::load_machine(machine_by_id("dragon")).unwrap();
    assert!(dragon.ante().is_some(), "dragon lost its ante");
}
