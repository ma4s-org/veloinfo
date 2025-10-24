local bicycle_route_ways = {}

local cycleway = osm2pgsql.define_way_table("cycleway_way", {{
    column = 'name',
    type = 'text'
}, {
    column = 'geom',
    type = 'LineString',
    not_null = true
}, {
    column = 'source',
    type = 'int8',
    not_null = true
}, {
    column = 'target',
    type = 'int8',
    not_null = true
}, {
    column = 'kind',
    type = 'text',
    not_null = true
}, {
    column = 'tags',
    type = 'jsonb',
    not_null = true
}, {
    column = 'is_conditionally_closed',
    type = 'boolean',
},{
    column = 'nodes',
    sql_type = 'int8[] NOT NULL'
}})

local cycleway_far = osm2pgsql.define_way_table("cycleway_way_far", {{
    column = 'name',
    type = 'text'
}, {
    column = 'geom',
    type = 'LineString',
    not_null = true
}, {
    column = 'source',
    type = 'int8',
    not_null = true
}, {
    column = 'target',
    type = 'int8',
    not_null = true
}, {
    column = 'kind',
    type = 'text',
    not_null = true
}, {
    column = 'tags',
    type = 'jsonb',
    not_null = true
}, {
    column = 'nodes',
    sql_type = 'int8[] NOT NULL'
}})

local all_way = osm2pgsql.define_way_table("all_way", {{
    column = 'name',
    type = 'text'
}, {
    column = 'geom',
    type = 'LineString',
    not_null = true
}, {
    column = 'source',
    type = 'int8',
    not_null = true
}, {
    column = 'target',
    type = 'int8',
    not_null = true
}, {
    column = 'tags',
    type = 'jsonb',
    not_null = true
}, {
    column = 'nodes',
    sql_type = 'int8[] NOT NULL'
}, 
{
    column = 'is_conditionally_closed',
    type = 'boolean',
}, {
    column = 'in_bicycle_route',
    type = 'boolean',
    not_null = true,
    default = false
}})

local landuse = osm2pgsql.define_table({
    name = 'landuse',
    ids = {
        type = 'area',
        id_column = 'way_id'
    },
    columns = {{
        column = 'name',
        type = 'text'
    }, {
        column = 'geom',
        type = 'multipolygon',
        not_null = true
    }, {
        column = 'tags',
        type = 'jsonb',
        not_null = true
    }, {
        column = 'landuse',
        type = 'text'
    }},
    indexes = {{
        column = 'geom',
        method = 'gist'
    }}
})

local city = osm2pgsql.define_table({
    name = 'city',
    ids = {
        type = 'area',
        id_column = 'way_id'
    },
    columns = {{
        column = 'name',
        type = 'text'
    }, {
        column = 'geom',
        type = 'multipolygon',
        not_null = true
    }, {
        column = 'tags',
        type = 'jsonb',
        not_null = true
    }, {
        column = 'admin_level',
        type = 'integer'
    }},
    indexes = {{
        column = 'geom',
        method = 'gist'
    },
    {
        column = 'name',
        method = 'btree'
    }}
})
local landcover = osm2pgsql.define_table({
    name = 'landcover',
    ids = {
        type = 'area',
        id_column = 'way_id'
    },
    columns = {{
        column = 'name',
        type = 'text'
    }, {
        column = 'geom',
        type = 'multipolygon',
        not_null = true
    }, {
        column = 'tags',
        type = 'jsonb',
        not_null = true
    }, {
        column = 'landuse',
        type = 'text'
    }, {
        column = 'natural',
        type = 'text'
    }, {
        column = 'leisure',
        type = 'text'
    }, {
        column = 'landcover',
        type = 'text'
    }, {
        column = 'waterway',
        type = 'text'
    }},
    indexes = {{
        column = 'geom',
        method = 'gist'
    }}
})

local landcover_far = osm2pgsql.define_table({
    name = 'landcover_far',
    ids = {
        type = 'area',
        id_column = 'way_id'
    },
    columns = {{
        column = 'name',
        type = 'text'
    }, {
        column = 'geom',
        type = 'multipolygon',
        not_null = true
    }, {
        column = 'tags',
        type = 'jsonb',
        not_null = true
    }, {
        column = 'landuse',
        type = 'text'
    }, {
        column = 'natural',
        type = 'text'
    }, {
        column = 'leisure',
        type = 'text'
    }, {
        column = 'landcover',
        type = 'text'
    }},
    indexes = {{
        column = 'geom',
        method = 'gist'
    }}
})

local water_name = osm2pgsql.define_table({
    name = 'water_name',
    ids = {
        type = 'node',
        id_column = 'node_id'
    },
    columns = {{
        column = 'name',
        type = 'text'
    }, {
        column = 'geom',
        type = 'point',
        not_null = true
    }, {
        column = 'tags',
        type = 'jsonb',
        not_null = true
    }, {
        column = 'place',
        type = 'text'
    }},
    indexes = {{
        column = 'geom',
        method = 'gist'
    }}
})

local aeroway = osm2pgsql.define_table({
    name = 'aeroway',
    ids = {
        type = 'area',
        id_column = 'way_id'
    },
    columns = {{
        column = 'name',
        type = 'text'
    }, {
        column = 'geom',
        type = 'LineString',
        not_null = true
    }, {
        column = 'tags',
        type = 'jsonb',
        not_null = true
    }, {
        column = 'aeroway',
        type = 'text'
    }},
    indexes = {{
        column = 'geom',
        method = 'gist'
    }}
})

local transportation = osm2pgsql.define_table({
    name = 'transportation',
    ids = {
        type = 'area',
        id_column = 'way_id'
    },
    columns = {{
        column = 'name',
        type = 'text'
    }, {
        column = 'name_fr',
        type = 'text'
    }, {
        column = 'geom',
        type = 'LineString',
        not_null = true
    }, {
        column = 'tags',
        type = 'jsonb',
        not_null = true
    }, {
        column = 'tunnel',
        type = 'text'
    }, {
        column = 'highway',
        type = 'text'
    }, {
        column = 'railway',
        type = 'text'
    }},
    indexes = {{
        column = 'geom',
        method = 'gist'
    }}
})

local building = osm2pgsql.define_table({
    name = 'building',
    ids = {
        type = 'area',
        id_column = 'way_id'
    },
    columns = {{
        column = 'name',
        type = 'text'
    }, {
        column = 'geom',
        type = 'multipolygon',
        not_null = true
    }, {
        column = 'tags',
        type = 'jsonb',
        not_null = true
    }, {
        column = 'building',
        type = 'text'
    }},
    indexes = {{
        column = 'geom',
        method = 'gist'
    }}
})

local boundary = osm2pgsql.define_table({
    name = 'boundary',
    ids = {
        type = 'area',
        id_column = 'way_id'
    },
    columns = {{
        column = 'name',
        type = 'text'
    }, {
        column = 'geom',
        type = 'LineString',
        not_null = true
    }, {
        column = 'tags',
        type = 'jsonb',
        not_null = true
    }, {
        column = 'boundary',
        type = 'text'
    }, {
        column = 'admin_level',
        type = 'integer'
    }},
    indexes = {{
        column = 'geom',
        method = 'gist'
    }}
})

local all_node = osm2pgsql.define_node_table('all_node', {{
    column = 'name',
    type = 'text'
}, {
    column = 'geom',
    type = 'Point'
}, {
    column = 'tags',
    type = 'jsonb'
}, {
    column = 'place',
    type = 'text'
}, {
    column = 'amenity',
    type = 'text'
}, {
    column = 'bicycle_parking',
    type = 'text'
}, {
    column = 'capacity',
    type = 'integer'
}, {
    column = 'shop',
    type = 'text'
}, {
    column = 'network',
    type = 'text'
}})

local address = osm2pgsql.define_table({
    name = 'address',
    ids = {
        type = 'area',
        id_column = 'way_id'
    },
    columns = {{
        column = 'geom',
        type = 'LineString',
        not_null = true
    }, {
        column = 'tags',
        type = 'jsonb',
        not_null = true
    }, {
        column = 'housenumber1',
        type = 'int8'
    }, {
        column = 'housenumber2',
        type = 'int8'
    }, {
        column = 'odd_even',
        type = 'text'
    }},
    indexes = {{
        column = 'geom',
        method = 'gist'
    }}
})

local address_node = osm2pgsql.define_node_table('address_node', {{
    column = 'geom',
    type = 'Point'
}, {
    column = 'tags',
    type = 'jsonb'
}, {
    column = 'city',
    type = 'text'
}, {
    column = 'street',
    type = 'text'
}, {
    column = 'housenumber',
    type = 'integer'
}})

local name = osm2pgsql.define_node_table('name', {{
    column = 'geom',
    type = 'Point'
}, {
    column = 'tags',
    type = 'jsonb'
}, {
    column = 'name',
    type = 'text'
}})

function month_str_to_number(month_str)
    local months = {
        jan = 1, feb = 2, mar = 3, apr = 4, may = 5, jun = 6,
        jul = 7, aug = 8, sep = 9, oct = 10, nov = 11, dec = 12
    }
    return months[month_str:sub(1,3)]
end

function is_conditionally_closed(tags)
    if not tags.conditional then
        return false
    end

    local conditional_access = string.lower(tags.conditional)
    local months_part = string.match(conditional_access, "no @ %((.-)%)")

    if not months_part then
        return false
    end

    local current_month = os.date("*t").month
    
    for month_range in string.gmatch(months_part, "[^,]+") do
        month_range = string.gsub(month_range, "%s+", "") -- remove spaces
        local start_month_str, end_month_str = string.match(month_range, "([a-z]+)-([a-z]+)")
        if start_month_str and end_month_str then
            local start_month = month_str_to_number(start_month_str)
            local end_month = month_str_to_number(end_month_str)
            if start_month and end_month then
                if start_month <= end_month then
                    if current_month >= start_month and current_month <= end_month then
                        return true
                    end
                else -- Handles ranges like Nov-Mar
                    if current_month >= start_month or current_month <= end_month then
                        return true
                    end
                end
            end
        else
            local month = month_str_to_number(month_range)
            if month and current_month == month then
                return true
            end
        end
    end

    return false
end

function osm2pgsql.process_way(way)
    if (way.tags.highway == 'cycleway' or way.tags.cyclestreet == "yes"  
        or way.tags.cycleway == "track" or way.tags["cycleway:left"] == "track" or 
        way.tags["cycleway:right"] == "track" or way.tags["cycleway:both"] =="track") 
        and way.tags.service ~= "parking_aisle" and way.tags.highway ~= "proposed" then
        cycleway:insert({
            name = way.tags.name,
            geom = way:as_linestring(),
            source = way.nodes[1],
            target = way.nodes[#way.nodes],
            kind = (way.tags.cycleway == 'crossing') and 'cycleway_crossing' or 'cycleway',
            tags = way.tags,
            is_conditionally_closed = is_conditionally_closed(way.tags),
            nodes = "{" .. table.concat(way.nodes, ",") .. "}"
        })
    elseif (way.tags["cycleway:left"] == "share_busway" or way.tags["cycleway:right"] == "share_busway" or
        way.tags["cycleway:both"] == "share_busway" or way.tags["cycleway:right"] == "lane" or way.tags["cycleway:left"] ==
        "lane" or way.tags["cycleway:both"] == "lane") and way.tags.service ~= "parking_aisle" then
        cycleway:insert({
            name = way.tags.name,
            geom = way:as_linestring(),
            source = way.nodes[1],
            target = way.nodes[#way.nodes],
            kind = (way.tags.cycleway == 'crossing') and 'designated_crossing' or 'designated',
            tags = way.tags,
            is_conditionally_closed = is_conditionally_closed(way.tags),
            nodes = " {" .. table.concat(way.nodes, ",") .. "}"
        })
    elseif (way.tags.cycleway == "shared_lane" or way.tags.cycleway == "lane" or way.tags["cycleway:left"] ==
        "shared_lane" or way.tags["cycleway:left"] == "opposite_lane" or way.tags["cycleway:right"] == "shared_lane" or
        way.tags["cycleway:right"] == "opposite_lane" or way.tags["cycleway:both"] == "shared_lane" or
        (way.tags.highway == "footway" and way.tags.bicycle == "yes")) and way.tags.service ~= "parking_aisle" then
        cycleway:insert({
            name = way.tags.name,
            geom = way:as_linestring(),
            source = way.nodes[1],
            target = way.nodes[#way.nodes],
            kind = (way.tags.cycleway == 'crossing') and 'shared_lane_crossing' or 'shared_lane',
            tags = way.tags,
            is_conditionally_closed = is_conditionally_closed(way.tags),
            nodes = "{" .. table.concat(way.nodes, ",") .. "}"
        })
    end

    if (way.tags.highway == 'cycleway') and way.tags.footway ~=
        "sidewalk" and way.tags.service ~= "parking_aisle" and way.tags.highway ~= "proposed" then
        cycleway_far:insert({
            name = way.tags.name,
            geom = way:as_linestring(),
            source = way.nodes[1],
            target = way.nodes[#way.nodes],
            kind = (way.tags.cycleway == 'crossing') and 'cycleway_crossing' or 'cycleway',
            tags = way.tags,
            nodes = "{" .. table.concat(way.nodes, ",") .. "}"
        })
    end

    if way.tags.building and way.tags.location ~= "underground" then
        building:insert({
            name = way.tags.name,
            geom = way:as_polygon(),
            tags = way.tags,
            building = way.tags.building
        })
    end

    if (way.tags.highway or way.tags.railway) and way.tags.footway ~= "sidewalk" and way.tags.highway ~= "steps" and
        way.tags.service ~= "parking_aisle" and way.tags.highway ~= "proposed" then
        transportation:insert({
            name = way.tags.name,
            name_fr = way.tags["name:fr"],
            geom = way:as_linestring(),
            tags = way.tags,
            tunnel = way.tags.tunnel,
            highway = way.tags.highway,
            railway = way.tags.railway

        })
    end

    if way.tags.aeroway then
        aeroway:insert({
            name = way.tags.name,
            geom = way:as_linestring(),
            tags = way.tags,
            aeroway = way.tags.aeroway
        })
    end

    if way.tags.highway and way.tags.service ~= "parking_aisle" and way.tags.highway ~= "proposed" then
        all_way:insert({
            name = way.tags.name,
            geom = way:as_linestring(),
            source = way.nodes[1],
            target = way.nodes[#way.nodes],
            tags = way.tags,
            nodes = "{" .. table.concat(way.nodes, ",") .. "}",
            is_conditionally_closed = is_conditionally_closed(way.tags),
            in_bicycle_route = bicycle_route_ways[way.id] or false
        })
    end

    if (way.tags.landuse == "forest" or way.tags.landuse == "cemetery" or way.tags.natural == "wood" or way.tags.natural ==
        "water" or way.tags.waterway or way.tags.leisure == "park" or way.tags.landuse == "residential") then
        landcover:insert({
            name = way.tags.name,
            geom = way:as_polygon(),
            tags = way.tags,
            landuse = way.tags.landuse,
            natural = way.tags.natural,
            leisure = way.tags.leisure,
            landcover = way.tags.landcover,
            waterway = way.tags.waterway
        })
    end

    if way.tags["addr:interpolation"] then
        address:insert({
            geom = way:as_linestring(),
            tags = way.tags,
            odd_even = way.tags["addr:interpolation"],
            housenumber1 = way.nodes[1],
            housenumber2 = way.nodes[#way.nodes]
        })
    end
end

function osm2pgsql.process_relation(relation)
    if relation.tags.landuse == "forest" or relation.tags.landuse == "cemetery" or relation.tags.natural == "wood" or
        relation.tags.natural == "water" or relation.tags.natural == "bay" or relation.tags.leisure == "park" or relation.tags.landuse == "residential" then
        landcover:insert({
            name = relation.tags.name,
            geom = relation:as_multipolygon(),
            tags = relation.tags,
            landuse = relation.tags.landuse,
            natural = relation.tags.natural,
            leisure = relation.tags.leisure,
            landcover = relation.tags.landcover
        })
    end
    if relation.tags.admin_level == "8" then
        city:insert({
            name = relation.tags.name,
            geom = relation:as_multipolygon(),
            tags = relation.tags,
            admin_level = relation.tags.admin_level
        })
    end
    if relation:as_multipolygon():area() > 1e-3 and
        (relation.tags.natural == "water" or relation.tags.natural == "bay" or relation.tags.landuse == "forest") then
        landcover_far:insert({
            name = relation.tags.name,
            geom = relation:as_multipolygon(),
            tags = relation.tags,
            landuse = relation.tags.landuse,
            natural = relation.tags.natural,
            leisure = relation.tags.leisure,
            landcover = relation.tags.landcover
        })
    end

    if relation.tags.building then
        building:insert({
            name = relation.tags.name,
            geom = relation:as_multipolygon(),
            tags = relation.tags,
            building = relation.tags.building
        })
    end
end

function osm2pgsql.process_node(node)
    if node.tags.place or node.tags.amenity or node.tags.shop == "bicycle" then
        all_node:insert({
            name = node.tags.name,
            geom = node:as_point(),
            tags = node.tags,
            place = node.tags.place,
            amenity = node.tags.amenity,
            network = node.tags.network,
            bicycle_parking = node.tags["bicycle_parking"],
            capacity = node.tags.capacity,
            shop = node.tags.shop
        })
    end

    if node.tags.place == "ocean" or node.tags.place == "sea" then
        water_name:insert({
            name = node.tags.name,
            geom = node:as_point(),
            tags = node.tags,
            place = node.tags.place
        })
    end

    if node.tags["addr:street"] then
        address_node:insert({
            geom = node:as_point(),
            tags = node.tags,
            city = node.tags["addr:city"],
            street = node.tags["addr:street"],
            housenumber = node.tags["addr:housenumber"]
        })
    end
    if node.tags.name then
        name:insert({
            name = node.tags.name,
            geom = node:as_point(),
            tags = node.tags
        })
    end
end

function osm2pgsql.select_relation_members(relation)
    if relation.tags.route == "bicycle" then
        for _, way_id in ipairs(osm2pgsql.way_member_ids(relation)) do
            bicycle_route_ways[way_id] = true
        end
        return {
            nodes = {},
            ways = osm2pgsql.way_member_ids(relation)
        }
    end

end

