use crate::db::edge::{
    Access, Bicycle, Cycleway, EdgePoint, Footway, Highway, Oneway, Smoothness, SourceOrTarget,
    Surface, Tunnel,
};
use crate::db::utils::distance_meters;
use crate::utils::elevation;

pub trait H: Send {
    fn get_cost(&self, edge: &EdgePoint) -> f64;
    fn get_max_point(&self) -> i64;

    fn h(&self, start_point: &EdgePoint, goal: &EdgePoint) -> f64 {
        let (goal_lon, goal_lat) = if SourceOrTarget::Source == goal.direction {
            (goal.lon1, goal.lat1)
        } else {
            (goal.lon2, goal.lat2)
        };
        let (start_lon, start_lat) = if SourceOrTarget::Source == start_point.direction {
            (start_point.lon1, start_point.lat1)
        } else {
            (start_point.lon2, start_point.lat2)
        };
        let distance = distance_meters(start_lat, start_lon, goal_lat, goal_lon);

        distance * 1.041
    }
}

pub fn get_h_moyen() -> Box<dyn H> {
    Box::new(HMoyen {})
}

pub fn get_h_bigger_selection() -> Box<dyn H> {
    Box::new(HBiggerSelection {})
}

pub fn get_h_rapid() -> Box<dyn H> {
    Box::new(HRapid {})
}

pub struct HMoyen {}

impl H for HMoyen {
    fn get_max_point(&self) -> i64 {
        i64::MAX
    }

    fn get_cost(&self, edge: &EdgePoint) -> f64 {
        get_cost(FastOrSafe::Safe, edge)
    }
}

pub struct HBiggerSelection {}

impl H for HBiggerSelection {
    fn get_cost(&self, edge: &EdgePoint) -> f64 {
        let cost = if edge.highway == Some(Highway::Cycleway) {
            1.0
        } else if edge.cyclestreet {
            1.0
        } else if edge.cycleway == Some(Cycleway::Crossing) {
            1.0
        } else if has_cycleway_of_type(edge, &Cycleway::Track) {
            1.011
        } else if has_cycleway_of_type(edge, &Cycleway::Lane) {
            1.025
        } else if has_cycleway_of_type(edge, &Cycleway::SharedLane)
            || has_cycleway_of_type(edge, &Cycleway::ShareBusway)
        {
            1.12
        } else if edge.highway == Some(Highway::Unclassified) {
            1.08
        } else if edge.bicycle == Some(Bicycle::Designated) {
            1.043
        } else {
            10.0
        };

        cost
    }

    fn get_max_point(&self) -> i64 {
        10_000
    }
}

pub struct HRapid {}

impl H for HRapid {
    fn get_cost(&self, edge: &EdgePoint) -> f64 {
        get_cost(FastOrSafe::Fast, edge)
    }

    fn get_max_point(&self) -> i64 {
        1000000000000000000
    }
}

enum FastOrSafe {
    Fast,
    Safe,
}

/// Vérifie si un type de cycleway est disponible dans la direction de voyage
/// Prend en compte: cycleway, cycleway_both, cycleway_left, cycleway_right
/// et les contraintes de sens (oneway)
fn has_cycleway_of_type(edge: &EdgePoint, cycleway_type: &Cycleway) -> bool {
    // cycleway s'applique toujours
    if edge.cycleway == Some(*cycleway_type) {
        return true;
    }

    // cycleway_both s'applique toujours
    if edge.cycleway_both == Some(*cycleway_type) {
        return true;
    }

    // cycleway_left: disponible si on voyage vers source OU si two-way
    if edge.cycleway_left == Some(*cycleway_type) {
        if SourceOrTarget::Source == edge.direction || edge.cycleway_left_oneway == Some(Oneway::No)
        {
            return true;
        }
    }

    // cycleway_right: disponible si on voyage vers target OU si two-way
    if edge.cycleway_right == Some(*cycleway_type) {
        if SourceOrTarget::Target == edge.direction
            || edge.cycleway_right_oneway == Some(Oneway::No)
        {
            return true;
        }
    }

    false
}

fn get_cycleway_cost(edge: &EdgePoint) -> Option<f64> {
    // Déterminer le type de cycleway et son coefficient
    let coefficient = if edge.highway == Some(Highway::Cycleway) {
        1.0
    } else if has_cycleway_of_type(edge, &Cycleway::Track) {
        1.0
    } else if has_cycleway_of_type(edge, &Cycleway::Lane) {
        1.2
    } else if has_cycleway_of_type(edge, &Cycleway::SharedLane) {
        1.3
    } else if has_cycleway_of_type(edge, &Cycleway::ShareBusway) {
        1.4
    } else {
        return None;
    };

    let mut base = 1.0;

    // Conditions exclusives (une seule s'applique)
    if edge.highway == Some(Highway::Cycleway)
        && (edge.surface == Some(Surface::FineGravel) || edge.surface == Some(Surface::Gravel))
    {
        base += 1.0;
    } else if edge.cycleway == Some(Cycleway::Crossing) {
        base += 1.0;
    } else if edge.smoothness == Some(Smoothness::Bad) {
        base += 1.0;
    } else if edge.surface == Some(Surface::Sett) {
        base += 1.0;
    }

    Some(base * coefficient)
}

fn get_local_road_cost(edge: &EdgePoint) -> Option<f64> {
    // Vérifier si c'est un local road et obtenir le coefficient
    let coefficient = if edge.cyclestreet {
        0.9
    } else {
        match edge.highway {
            Some(Highway::Residential) | Some(Highway::LivingStreet) => 1.0,
            Some(Highway::Unclassified) => 1.05,
            Some(Highway::Service) => 1.05,
            Some(Highway::Tertiary) => 1.1,
            Some(Highway::Secondary) | Some(Highway::SecondaryLink) => 1.3,
            _ => return None,
        }
    };

    let mut base = 1.8;

    // Conditions positives : exclusives (une seule s'applique)
    if edge.surface == Some(Surface::Sett)
        || edge.surface == Some(Surface::Cobblestone)
        || edge.surface == Some(Surface::UnhewnCobblestone)
    {
        base += 3.7;
    } else if edge.surface == Some(Surface::Chipseal) {
        base += 0.5;
    }

    // Conditions cumulables
    if edge.tunnel == Some(Tunnel::Yes) {
        base += 0.5;
    }
    if edge.bicycle == Some(Bicycle::Yes) || edge.bicycle == Some(Bicycle::Designated) {
        base -= 0.4;
    }
    if edge.bicycle == Some(Bicycle::Dismount) {
        base += 2.0;
    }
    if edge.in_bicycle_route {
        base -= 0.2;
    }

    Some(base * coefficient)
}

fn get_cost(fast_or_safe: FastOrSafe, edge: &EdgePoint) -> f64 {
    // if the target is the source we are reverse of the edge
    if SourceOrTarget::Source == edge.direction
        && (edge.oneway == Some(Oneway::Yes)
            && edge.oneway_bicycle != Some(Oneway::No)
            && edge.cycleway_left_oneway != Some(Oneway::No)
            && edge.cycleway_right_oneway != Some(Oneway::No)
            || edge.cycleway_left == Some(Cycleway::Snow))
    {
        return 10000.0;
    }

    if edge.winter_service_no && edge.snow {
        return 10000.0;
    }

    if edge.bicycle == Some(Bicycle::No) {
        return 10000.0;
    }

    if edge.highway == Some(Highway::Proposed)
        || edge.abandoned
        || edge.highway == Some(Highway::Motorway)
        || edge.highway == Some(Highway::Construction)
    {
        return 10000.0;
    }

    if (edge.access == Some(Access::Private) || edge.access == Some(Access::No) || edge.informal)
        && edge.bicycle != Some(Bicycle::Yes)
    {
        return 10000.0;
    }

    let mut cost: f64 = if edge.highway == Some(Highway::Steps) {
        if edge.bicycle == Some(Bicycle::Yes) || edge.bicycle == Some(Bicycle::Designated) {
            15.0
        } else {
            return 10000.0;
        }
    } else if edge.highway == Some(Highway::Path) {
        if edge.bicycle == Some(Bicycle::Yes) {
            1.1
        } else if edge.bicycle == Some(Bicycle::Dismount)
            || edge.bicycle == Some(Bicycle::Discouraged)
        {
            2.5
        } else {
            10.0
        }
    } else if edge.bicycle == Some(Bicycle::Discouraged) {
        10.0
    } else if edge.routing_bicycle_use_sidepath {
        10.0
    } else if let Some(cycleway_cost) = get_cycleway_cost(edge) {
        cycleway_cost
    } else if edge.highway == Some(Highway::Footway) || edge.highway == Some(Highway::Pedestrian) {
        if edge.bicycle == Some(Bicycle::Yes) || edge.bicycle == Some(Bicycle::Designated) {
            if edge.footway == Some(Footway::Sidewalk) {
                2.5
            } else {
                1.4
            }
        } else if edge.bicycle == Some(Bicycle::Dismount) {
            if edge.tunnel == Some(Tunnel::Yes) || edge.footway == Some(Footway::Sidewalk) {
                20.0
            } else {
                if edge.bridge == Some(true) {
                    1.6
                } else {
                    5.
                }
            }
        } else if edge.footway == Some(Footway::Sidewalk) {
            15.0
        } else if edge.footway == Some(Footway::Crossing) {
            50.0
        } else {
            15.0
        }
    } else if let Some(local_cost) = get_local_road_cost(edge) {
        local_cost
    } else if edge.highway == Some(Highway::Primary) {
        if edge.in_bicycle_route {
            2.
        } else {
            3.5
        }
    } else if edge.highway == Some(Highway::Trunk) {
        9.0
    } else if edge.highway.is_some() {
        10.0
    } else {
        20.0
    };

    if edge.access == Some(Access::Customers) {
        cost = cost + 5.0;
    }

    if edge.road_work {
        cost *= 10.0;
    }

    cost = match fast_or_safe {
        FastOrSafe::Fast => {
            cost = cost * elevation::get_edge_slope_cost(edge);
            1.0 + cost.log(20.0)
        }
        FastOrSafe::Safe => {
            let slope_cost = elevation::get_edge_slope_cost(edge);
            cost * slope_cost
        }
    };

    let score = match edge.score {
        Some(score) => {
            if score == -1.00 {
                1.0
            } else if score == 0.0 {
                0.01
            } else {
                score
            }
        }
        None => 1.0,
    };
    cost / score
}

#[cfg(test)]
mod tests {
    use crate::db::edge::{EdgePoint, SourceOrTarget};
    use crate::utils::cost::{get_cost, FastOrSafe};

    #[test]
    fn test_get_cost() {
        let edge = EdgePoint {
            direction: SourceOrTarget::Source,
            ..EdgePoint::default()
        };
        let cost = get_cost(FastOrSafe::Safe, &edge);
        assert_eq!(cost, 20.0);
    }

    #[test]
    fn test_olmstead() {
        let edge = EdgePoint {
            bicycle: Some(crate::db::edge::Bicycle::Designated),
            highway: Some(crate::db::edge::Highway::Pedestrian),
            surface: Some(crate::db::edge::Surface::Gravel),
            ..EdgePoint::default()
        };
        let cost = get_cost(FastOrSafe::Safe, &edge);
        assert_eq!(cost, 1.1);
    }
}
