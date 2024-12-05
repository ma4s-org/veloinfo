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
        type = 'area',
        id_column = 'way_id'
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

function osm2pgsql.process_way(object)
    if (object.tags.highway == 'cycleway' or object.tags.cyclestreet == "yes" or object.tags.cycleway == "track" or
        object.tags["cycleway:left"] == "track" or object.tags["cycleway:right"] == "track" or
        object.tags["cycleway:both"] == "track") and object.tags.service ~= "parking_aisle" and object.tags.highway ~=
        "proposed" then
        cycleway:insert({
            name = object.tags.name,
            geom = object:as_linestring(),
            source = object.nodes[1],
            target = object.nodes[#object.nodes],
            kind = 'cycleway',
            tags = object.tags,
            nodes = "{" .. table.concat(object.nodes, ",") .. "}"
        })
    elseif (object.tags["cycleway:left"] == "share_busway" or object.tags["cycleway:right"] == "share_busway" or
        object.tags["cycleway:both"] == "share_busway" or object.tags["cycleway:right"] == "lane" or
        object.tags["cycleway:left"] == "lane" or object.tags["cycleway:both"] == "lane") and object.tags.service ~=
        "parking_aisle" then
        cycleway:insert({
            name = object.tags.name,
            geom = object:as_linestring(),
            source = object.nodes[1],
            target = object.nodes[#object.nodes],
            kind = 'designated',
            tags = object.tags,
            nodes = " {" .. table.concat(object.nodes, ",") .. "}"
        })
    elseif (object.tags.cycleway == "shared_lane" or object.tags.cycleway == "lane" or object.tags["cycleway:left"] ==
        "shared_lane" or object.tags["cycleway:left"] == "opposite_lane" or object.tags["cycleway:right"] ==
        "shared_lane" or object.tags["cycleway:right"] == "opposite_lane" or object.tags["cycleway:both"] ==
        "shared_lane" or (object.tags.highway == "footway" and object.tags.bicycle == "yes")) and object.tags.service ~=
        "parking_aisle" then
        cycleway:insert({
            name = object.tags.name,
            geom = object:as_linestring(),
            source = object.nodes[1],
            target = object.nodes[#object.nodes],
            kind = 'shared_lane',
            tags = object.tags,
            nodes = "{" .. table.concat(object.nodes, ",") .. "}"
        })
    end

    if (object.tags.highway == 'cycleway' or object.tags.cycleway == "track" or object.tags["cycleway:left"] == "track" or
        object.tags["cycleway:right"] == "track" or object.tags["cycleway:both"] == "track") and object.tags.footway ~=
        "sidewalk" and object.tags.service ~= "parking_aisle" and object.tags.highway ~= "proposed" then
        cycleway_far:insert({
            name = object.tags.name,
            geom = object:as_linestring(),
            source = object.nodes[1],
            target = object.nodes[#object.nodes],
            kind = 'cycleway',
            tags = object.tags,
            nodes = "{" .. table.concat(object.nodes, ",") .. "}"
        })
    end

    if object.tags.building and object.tags.location ~= "underground" then
        building:insert({
            name = object.tags.name,
            geom = object:as_polygon(),
            tags = object.tags,
            building = object.tags.building
        })
    end

    if (object.tags.highway or object.tags.railway) and object.tags.footway ~= "sidewalk" and object.tags.highway ~=
        "steps" and object.tags.service ~= "parking_aisle" and object.tags.highway ~= "proposed" then
        transportation:insert({
            name = object.tags.name,
            name_fr = object.tags["name:fr"],
            geom = object:as_linestring(),
            tags = object.tags,
            tunnel = object.tags.tunnel,
            highway = object.tags.highway,
            railway = object.tags.railway

        })
    end

    if object.tags.aeroway then
        aeroway:insert({
            name = object.tags.name,
            geom = object:as_linestring(),
            tags = object.tags,
            aeroway = object.tags.aeroway
        })
    end

    if object.tags.highway and object.tags.service ~= "parking_aisle" and object.tags.highway ~= "proposed" then
        all_way:insert({
            name = object.tags.name,
            geom = object:as_linestring(),
            source = object.nodes[1],
            target = object.nodes[#object.nodes],
            tags = object.tags,
            nodes = "{" .. table.concat(object.nodes, ",") .. "}"
        })
    end

    if (object.tags.landuse == "forest" or object.tags.landuse == "cemetery" or object.tags.natural == "wood" or
        object.tags.natural == "water" or object.tags.waterway or object.tags.leisure == "park" or object.tags.landuse ==
        "residential") then
        landcover:insert({
            name = object.tags.name,
            geom = object:as_polygon(),
            tags = object.tags,
            landuse = object.tags.landuse,
            natural = object.tags.natural,
            leisure = object.tags.leisure,
            landcover = object.tags.landcover,
            waterway = object.tags.waterway
        })
    end

    if object.tags["addr:interpolation"] then
        address:insert({
            geom = object:as_linestring(),
            tags = object.tags,
            odd_even = object.tags["addr:interpolation"],
            housenumber1 = object.nodes[1],
            housenumber2 = object.nodes[#object.nodes]
        })
    end
end

function osm2pgsql.process_relation(object)
    if object.tags.landuse == "forest" or object.tags.landuse == "cemetery" or object.tags.natural == "wood" or
        object.tags.natural == "water" or object.tags.leisure == "park" or object.tags.landuse == "residential" then
        landcover:insert({
            name = object.tags.name,
            geom = object:as_multipolygon(),
            tags = object.tags,
            landuse = object.tags.landuse,
            natural = object.tags.natural,
            leisure = object.tags.leisure,
            landcover = object.tags.landcover
        })
    end
    if object:as_multipolygon():area() > 1e-3 and (object.tags.natural == "water" or object.tags.landuse == "forest") then
        landcover_far:insert({
            name = object.tags.name,
            geom = object:as_multipolygon(),
            tags = object.tags,
            landuse = object.tags.landuse,
            natural = object.tags.natural,
            leisure = object.tags.leisure,
            landcover = object.tags.landcover
        })
    end

    if object.tags.building then
        building:insert({
            name = object.tags.name,
            geom = object:as_multipolygon(),
            tags = object.tags,
            building = object.tags.building
        })
    end

end

function osm2pgsql.process_node(object)
    if object.tags.place or object.tags.amenity or object.tags.shop == "bicycle" then
        all_node:insert({
            name = object.tags.name,
            geom = object:as_point(),
            tags = object.tags,
            place = object.tags.place,
            amenity = object.tags.amenity,
            network = object.tags.network,
            bicycle_parking = object.tags["bicycle_parking"],
            capacity = object.tags.capacity,
            shop = object.tags.shop
        })
    end

    if object.tags.place == "ocean" or object.tags.place == "sea" then
        water_name:insert({
            name = object.tags.name,
            geom = object:as_point(),
            tags = object.tags,
            place = object.tags.place
        })
    end

    if object.tags["addr:street"] then
        address_node:insert({
            geom = object:as_point(),
            tags = object.tags,
            city = object.tags["addr:city"],
            street = object.tags["addr:street"],
            housenumber = object.tags["addr:housenumber"]
        })
    end
    if object.tags.name then
        name:insert({
            name = object.tags.name,
            geom = object:as_point(),
            tags = object.tags
        })
    end
end
