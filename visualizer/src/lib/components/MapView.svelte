<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import type { RouteData } from '$lib/types';
	import { getRouteGeometry, getStopPositions } from '$lib/parsers/routeData';
	import { projectCmToLatLon } from '$lib/parsers/projection';
	import maplibregl from 'maplibre-gl';
	import 'maplibre-gl/dist/maplibre-gl.css';
	import { FSM_STATE_COLORS } from '$lib/constants/fsmColors';
	import { getStopLatLon } from '$lib/parsers/routeData';
	import type { FsmState } from '$lib/types';

	// Constants for 50m circles
	const EARTH_RADIUS = 6378137; // meters
	const STOP_RADIUS_M = 50; // 50 meters

	/**
	 * Convert meters to pixels at a given latitude and zoom level
	 * Used for MapLibre GL circle radius calculations
	 */
	function metersToPixels(meters: number, lat: number, zoom: number): number {
		const latRad = lat * Math.PI / 180;
		const metersPerPixel = EARTH_RADIUS * Math.cos(latRad) / (256 * Math.pow(2, zoom));
		return meters / metersPerPixel;
	}

	/**
	 * Get stop lat/lon by interpolating along route nodes
	 * Wrapper for getStopLatLon that handles the grid_origin parameter
	 */
	function getStopPosition(stopProgressCm: number, routeData: RouteData): [number, number] | null {
		return getStopLatLon(stopProgressCm, routeData);
	}

	interface Props {
		routeData: RouteData;
		busPosition?: { lat: number; lon: number; heading?: number } | null;
		selectedStop?: number | null;
		onStopClick?: (stopIndex: number) => void;
		highlightedEvent?: {
			stopIdx: number;
			state: FsmState;
			time: number;
		} | null;
	}

	let {
		routeData,
		busPosition = null,
		selectedStop = null,
		onStopClick = () => {},
		highlightedEvent = null
	}: Props = $props();

	let mapContainer: HTMLDivElement;
	let map: maplibregl.Map | null = null;
	let mapLoaded = false;
	let routeSourceId = 'route';
	let stopsSourceId = 'stops';
	let busSourceId = 'bus';
	let currentPanTarget = $state<number | null>(null);

	onMount(() => {
		// Initialize map with OSM raster tiles (more reliable)
		const [initialLat, initialLon] = [25.0, 121.0]; // Taiwan center

		map = new maplibregl.Map({
			container: mapContainer,
			style: {
				version: 8,
				sources: {
					'osm-tiles': {
						type: 'raster',
						tiles: ['https://tile.openstreetmap.org/{z}/{x}/{y}.png'],
						tileSize: 256,
						attribution: '© OpenStreetMap contributors'
					}
				},
				layers: [
					{
						id: 'osm-tiles',
						type: 'raster',
						source: 'osm-tiles',
						minzoom: 0,
						maxzoom: 19
					}
				]
			},
			center: [initialLon, initialLat],
			zoom: 13
		});

		map.on('load', async () => {
			if (!map) return;

			// Add custom arrow icon for bus
			const svgArrow = `
				<svg width="40" height="40" viewBox="0 0 40 40" xmlns="http://www.w3.org/2000/svg">
					<circle cx="20" cy="20" r="18" fill="#22c55e" stroke="white" stroke-width="3"/>
					<path d="M20 5 L32 30 L20 22 L8 30 Z" fill="white"/>
				</svg>
			`;
			const blob = new Blob([svgArrow], { type: 'image/svg+xml' });
			const url = URL.createObjectURL(blob);
			const image = await new Promise<HTMLImageElement>((resolve) => {
				const img = new Image();
				img.onload = () => resolve(img);
				img.src = url;
			});
			map.addImage('bus-arrow', image);
			
			mapLoaded = true;

			// Add route line
			const routeGeo = getRouteGeometry(routeData, projectCmToLatLon);
			map.addSource(routeSourceId, {
				type: 'geojson',
				data: {
					type: 'Feature',
					properties: {},
					geometry: {
						type: 'LineString',
						coordinates: routeGeo
					}
				}
			});

			map.addLayer({
				id: 'route-line',
				type: 'line',
				source: routeSourceId,
				layout: {
					'line-join': 'round',
					'line-cap': 'round'
				},
				paint: {
					'line-color': '#3b82f6',
					'line-width': 4,
					'line-opacity': 0.8
				}
			});

			// Add stops
			const stops = getStopPositions(routeData, projectCmToLatLon);
			const stopFeatures: any[] = stops.map((stop) => ({
				type: 'Feature',
				properties: {
					index: stop.index,
					progress_cm: stop.progress_cm
				},
				geometry: {
					type: 'Point',
					coordinates: [stop.lon, stop.lat]
				}
			}));

			map.addSource(stopsSourceId, {
				type: 'geojson',
				data: {
					type: 'FeatureCollection',
					features: stopFeatures
				}
			});

			map.addLayer({
				id: 'stops-circle',
				type: 'circle',
				source: stopsSourceId,
				paint: {
					'circle-radius': 8,
					'circle-color': '#ef4444',
					'circle-stroke-width': 2,
					'circle-stroke-color': '#ffffff'
				}
			});

			// Add stop numbers
			map.addLayer({
				id: 'stops-label',
				type: 'symbol',
				source: stopsSourceId,
				layout: {
					'text-field': ['get', 'index'],
					'text-font': ['Open Sans Regular', 'Arial Unicode MS Regular'],
					'text-size': 12,
					'text-anchor': 'top',
					'text-offset': [0, 0.5]
				},
				paint: {
					'text-color': '#000000',
					'text-halo-color': '#ffffff',
					'text-halo-width': 2
				}
			});

			// Add click handler for stops
			map.on('click', 'stops-circle', (e) => {
				if (e.features && e.features[0]) {
					const stopIndex = e.features[0].properties?.index as number;
					onStopClick(stopIndex);
				}
			});

			map.on('mouseenter', 'stops-circle', () => {
				if (map) map.getCanvas().style.cursor = 'pointer';
			});

			map.on('mouseleave', 'stops-circle', () => {
				if (map) map.getCanvas().style.cursor = '';
			});

			// Fit map to route bounds
			if (routeGeo.length > 0) {
				const bounds = routeGeo.reduce(
					(bounds, coord) => bounds.extend(coord as [number, number]),
					new maplibregl.LngLatBounds(routeGeo[0] as [number, number], routeGeo[0] as [number, number])
				);
				map.fitBounds(bounds, { padding: 50 });
			}
		});

		return () => {
			if (map) {
				map.remove();
				map = null;
			}
		};
	});

	// Update bus position when it changes
	$effect(() => {
		if (!map || !busPosition || !mapLoaded) return;

		const { lat, lon } = busPosition;

		if (map.getSource(busSourceId)) {
			(map.getSource(busSourceId) as maplibregl.GeoJSONSource).setData({
				type: 'Feature',
				properties: {
					rotation: busPosition.heading || 0
				},
				geometry: {
					type: 'Point',
					coordinates: [lon, lat]
				}
			});
		} else {
			map.addSource(busSourceId, {
				type: 'geojson',
				data: {
					type: 'Feature',
					properties: {
						rotation: busPosition.heading || 0
					},
					geometry: {
						type: 'Point',
						coordinates: [lon, lat]
					}
				}
			});

			map.addLayer({
				id: 'bus-marker',
				type: 'symbol',
				source: busSourceId,
				layout: {
					'icon-image': 'bus-arrow',
					'icon-size': 0.8,
					'icon-rotate': ['get', 'rotation'],
					'icon-allow-overlap': true,
					'icon-ignore-placement': true,
					'icon-rotation-alignment': 'map'
				}
			});
		}
	});

	// Highlight selected stop
	$effect(() => {
		if (!map || !mapLoaded) return;

		if (selectedStop !== null) {
			map.setPaintProperty('stops-circle', 'circle-radius', [
				'match',
				['get', 'index'],
				selectedStop,
				12,
				8
			]);
			map.setPaintProperty('stops-circle', 'circle-color', [
				'match',
				['get', 'index'],
				selectedStop,
				'#f59e0b',
				'#ef4444'
			]);
		} else {
			map.setPaintProperty('stops-circle', 'circle-radius', 8);
			map.setPaintProperty('stops-circle', 'circle-color', '#ef4444');
		}
	});

	onDestroy(() => {
		if (map) {
			map.remove();
			map = null;
		}
	});
</script>

<div class="map-container">
	<div bind:this={mapContainer} class="maplibre-gl-map"></div>
</div>

<style>
	.map-container {
		width: 100%;
		height: 100%;
		position: relative;
	}

	.maplibre-gl-map {
		width: 100%;
		height: 100%;
	}
</style>
