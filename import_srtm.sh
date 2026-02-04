#!/bin/bash
# Script to import SRTM elevation data
# This script loads SRTM GeoTIFF data into PostGIS

set -e

SRTM_FILE="/tmp/srtm_22_03/srtm_22_03.tif"
SRTM_URL="https://srtm.csi.cgiar.org/wp-content/uploads/files/srtm_5x5/TIFF/srtm_22_03.zip"

# Check if SRTM file exists, if not download it
if [ ! -f "$SRTM_FILE" ]; then
    echo "SRTM file not found, downloading..."
    mkdir -p /tmp/srtm_22_03
    
    # Download the zip file
    echo "Downloading SRTM data from $SRTM_URL..."
    wget -O /tmp/srtm_22_03.zip "$SRTM_URL"
    
    # Extract the zip file
    echo "Extracting SRTM data..."
    unzip -o /tmp/srtm_22_03.zip -d /tmp/srtm_22_03/
    
    # Clean up zip file
    rm /tmp/srtm_22_03.zip
    
    echo "✓ SRTM data downloaded and extracted"
else
    echo "✓ SRTM file already exists"
fi

# Parse DATABASE_URL (format: postgresql://user:password@host:port/dbname)
if [ -z "$DATABASE_URL" ]; then
    echo "Error: DATABASE_URL environment variable is not set"
    exit 1
fi

# Remove protocol
local_url="${DATABASE_URL##*://}"

# Extract user (with or without password)
user_part="${local_url%%@*}"

if [[ "$user_part" == *":"* ]]; then
    # Has password
    DB_USER="${user_part%%:*}"
    DB_PASSWORD="${user_part##*:}"
else
    # No password
    DB_USER="$user_part"
    DB_PASSWORD=""
fi

# Extract host and port and database
remainder="${local_url##*@}"
DB_HOST="${remainder%%:*}"
DB_PORT="${remainder##*:}"
DB_PORT="${DB_PORT%%/*}"
DB_NAME="${remainder##*/}"

echo "SRTM Elevation Data Import Script"
echo "===================================="
echo ""
echo "Configuration:"
echo "  Host: $DB_HOST"
echo "  Port: $DB_PORT"
echo "  Database: $DB_NAME"
echo "  User: $DB_USER"
echo ""

echo "✓ raster2pgsql is installed"
echo ""

# Enable PostGIS Raster extension
echo "Enabling PostGIS Raster extension..."
if [ -z "$DB_PASSWORD" ]; then
    psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "CREATE EXTENSION IF NOT EXISTS postgis_raster;" 2>/dev/null || echo "Note: PostGIS Raster extension may already be enabled or requires superuser privileges"
else
    PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "CREATE EXTENSION IF NOT EXISTS postgis_raster;" 2>/dev/null || echo "Note: PostGIS Raster extension may already be enabled or requires superuser privileges"
fi
echo "✓ PostGIS Raster extension ready"
echo ""

# Generate SQL script
echo "Generating SQL script from GeoTIFF (tiled for performance)..."
SQL_FILE="/tmp/srtm_load.sql"
# -t: tile size, -d: drop table, -I: create spatial index, -C: add constraints, -M: analyze
raster2pgsql -t 256x256 -d -I -C -M "$SRTM_FILE" public.srtm_elevation > "$SQL_FILE"
echo "✓ SQL script generated at $SQL_FILE"
echo ""

# Load into database
echo "Loading SRTM data into PostgreSQL..."
if [ -z "$DB_PASSWORD" ]; then
    if ! psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -f "$SQL_FILE" 2>&1 | grep -v "perl: warning"; then
        echo "Error loading SRTM data!"
        exit 1
    fi
else
    if ! PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -f "$SQL_FILE" 2>&1 | grep -v "perl: warning"; then
        echo "Error loading SRTM data!"
        exit 1
    fi
fi

# Check if load was successful
if [ -z "$DB_PASSWORD" ]; then
    TABLE_EXISTS=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'srtm_elevation');" 2>/dev/null | xargs)
else
    TABLE_EXISTS=$(PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'srtm_elevation');" 2>/dev/null | xargs)
fi

if [ "$TABLE_EXISTS" != "t" ]; then
    echo "✗ Error: SRTM table was not created successfully"
    echo "Please ensure PostGIS Raster extension is installed and enabled"
    exit 1
fi

echo "✓ SRTM data loaded successfully"
echo "Optimizing raster indexes..."
if [ -z "$DB_PASSWORD" ]; then
    psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "CREATE INDEX IF NOT EXISTS srtm_elevation_rast_gist ON srtm_elevation USING GIST (ST_ConvexHull(rast));"
    psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "ANALYZE srtm_elevation;"
else
    PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "CREATE INDEX IF NOT EXISTS srtm_elevation_rast_gist ON srtm_elevation USING GIST (ST_ConvexHull(rast));"
    PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -c "ANALYZE srtm_elevation;"
fi
echo "✓ Raster index and stats updated"

echo "Building polygon elevation table (this can be VERY large and may take a while)..."
    if [ -z "$DB_PASSWORD" ]; then
        psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" << 'EOF'
DROP TABLE IF EXISTS srtm_elevation_polygons;
CREATE TABLE srtm_elevation_polygons (
    geom geometry(Polygon, 4326),
    elevation double precision
);
INSERT INTO srtm_elevation_polygons (geom, elevation)
SELECT (p).geom, (p).val
FROM (
    SELECT ST_PixelAsPolygons(rast, 1) AS p
    FROM srtm_elevation
) AS polys;
CREATE INDEX srtm_elevation_polygons_geom_idx ON srtm_elevation_polygons USING GIST (geom);
ANALYZE srtm_elevation_polygons;
EOF
    else
        PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" << 'EOF'
DROP TABLE IF EXISTS srtm_elevation_polygons;
CREATE TABLE srtm_elevation_polygons (
    geom geometry(Polygon, 4326),
    elevation double precision
);
INSERT INTO srtm_elevation_polygons (geom, elevation)
SELECT (p).geom, (p).val
FROM (
    SELECT ST_PixelAsPolygons(rast, 1) AS p
    FROM srtm_elevation
) AS polys;
CREATE INDEX srtm_elevation_polygons_geom_idx ON srtm_elevation_polygons USING GIST (geom);
ANALYZE srtm_elevation_polygons;
EOF
    fi
    echo "✓ Polygon elevation table built"
fi
echo ""

# Verify the data
echo "Verifying SRTM data..."
if [ -z "$DB_PASSWORD" ]; then
    RASTER_COUNT=$(psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM srtm_elevation;")
else
    RASTER_COUNT=$(PGPASSWORD="$DB_PASSWORD" psql -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" -t -c "SELECT COUNT(*) FROM srtm_elevation;")
fi

echo "✓ Rasters in database: $RASTER_COUNT"

