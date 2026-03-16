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

	// Constants for stop arrival zone circles (50m radius)
	const EARTH_RADIUS = 6378137; // meters
	const STOP_RADIUS_M = 50; // 50 meters - arrival detection threshold

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
		onClearHighlight?: () => void;
	}

	let {
		routeData,
		busPosition = null,
		selectedStop = null,
		onStopClick = () => {},
		highlightedEvent = null,
		onClearHighlight = () => {}
	}: Props = $props();

	let mapContainer: HTMLDivElement;
	let map: maplibregl.Map | null = null;
	let mapLoaded = false;
	let routeSourceId = 'route';
	let stopsSourceId = 'stops';
	let busSourceId = 'bus';
	let currentPanTarget = $state<number | null>(null);
	let handleKeyDownRef: ((e: KeyboardEvent) => void) | null = null;

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

		// Add scale control
		map.addControl(new maplibregl.ScaleControl({
			maxWidth: 100,
			unit: 'metric'
		}));

		map.on('load', async () => {
			if (!map) return;
			const mapRef = map;

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
			mapRef.addImage('bus-arrow', image);
			
			mapLoaded = true;

			// Add route line
			const routeGeo = getRouteGeometry(routeData, projectCmToLatLon);
			mapRef.addSource(routeSourceId, {
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

			mapRef.addLayer({
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

			mapRef.addSource(stopsSourceId, {
				type: 'geojson',
				data: {
					type: 'FeatureCollection',
					features: stopFeatures
				}
			});

			mapRef.addLayer({
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

			mapRef.addLayer({
				id: 'stops-accuracy-circles',
				type: 'circle',
				source: stopsSourceId,
				paint: {
					'circle-radius': [
						'interpolate',
						['linear'],
						['zoom'],
						12, metersToPixels(STOP_RADIUS_M, routeData.lat_avg_deg, 12),
						14, metersToPixels(STOP_RADIUS_M, routeData.lat_avg_deg, 14),
						16, metersToPixels(STOP_RADIUS_M, routeData.lat_avg_deg, 16),
						18, metersToPixels(STOP_RADIUS_M, routeData.lat_avg_deg, 18)
					],
					'circle-color': '#3b82f6',
					'circle-opacity': 0.15,
					'circle-stroke-width': 1,
					'circle-stroke-color': '#3b82f6',
					'circle-stroke-opacity': 0.3
				}
			});

			mapRef.addSource('event-marker', {
				type: 'geojson',
				data: { type: 'FeatureCollection', features: [] }
			});

			mapRef.addLayer({
				id: 'event-marker-pulse',
				type: 'circle',
				source: 'event-marker',
				paint: {
					'circle-radius': 20,
					'circle-color': ['get', 'color'],
					'circle-opacity': 0.3
				}
			});

			mapRef.addLayer({
				id: 'event-marker',
				type: 'circle',
				source: 'event-marker',
				paint: {
					'circle-radius': 12,
					'circle-color': ['get', 'color'],
					'circle-stroke-width': 3,
					'circle-stroke-color': '#ffffff',
					'circle-opacity': 1
				}
			});

			// Add stop numbers
			mapRef.addLayer({
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
			mapRef.on('click', 'stops-circle', (e) => {
				if (e.features && e.features[0]) {
					const stopIndex = e.features[0].properties?.index as number;
					onStopClick(stopIndex);
				}
			});

			mapRef.on('click', (e) => {
				const features = mapRef.queryRenderedFeatures(e.point, { layers: ['stops-circle'] });
				if (features.length === 0 && onClearHighlight) {
					onClearHighlight();
				}
			});

			mapRef.on('mouseenter', 'stops-circle', () => {
				mapRef.getCanvas().style.cursor = 'pointer';
			});

			mapRef.on('mouseleave', 'stops-circle', () => {
				mapRef.getCanvas().style.cursor = '';
			});

			// Fit map to route bounds
			if (routeGeo.length > 0) {
				const bounds = routeGeo.reduce(
					(bounds, coord) => bounds.extend(coord as [number, number]),
					new maplibregl.LngLatBounds(routeGeo[0] as [number, number], routeGeo[0] as [number, number])
				);
				mapRef.fitBounds(bounds, { padding: 50 });
			}
		});

		handleKeyDownRef = (e: KeyboardEvent) => {
			if (e.key === 'Escape' && onClearHighlight) {
				onClearHighlight();
			}
		};
		document.addEventListener('keydown', handleKeyDownRef);

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

	$effect(() => {
		if (!map || !mapLoaded) return;

		if (!highlightedEvent) {
			(map.getSource('event-marker') as maplibregl.GeoJSONSource).setData({
				type: 'FeatureCollection',
				features: []
			});
			return;
		}

		const stop = routeData.stops[highlightedEvent.stopIdx];
		if (!stop) return;

		const latLon = getStopPosition(stop.progress_cm, routeData);
		if (!latLon) return;

		const color = FSM_STATE_COLORS[highlightedEvent.state];

		(map.getSource('event-marker') as maplibregl.GeoJSONSource).setData({
			type: 'FeatureCollection',
			features: [{
				type: 'Feature',
				properties: { color },
				geometry: {
					type: 'Point',
					coordinates: [latLon[1], latLon[0]]
				}
			}]
		});
	});

	export function panToStop(stopIdx: number) {
		currentPanTarget = stopIdx;
	}

	$effect(() => {
		if (!map || !mapLoaded || currentPanTarget === null) return;

		const stop = routeData.stops[currentPanTarget];
		if (!stop) return;

		const latLon = getStopPosition(stop.progress_cm, routeData);
		if (!latLon) return;

		map.easeTo({
			center: [latLon[1], latLon[0]],
			zoom: 16,
			duration: 500
		});

		currentPanTarget = null;
	});

	// Highlight selected stop and filter 50m circles
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
			// Filter accuracy circles to only show selected stop
			map.setFilter('stops-accuracy-circles', ['==', ['get', 'index'], selectedStop]);
		} else {
			map.setPaintProperty('stops-circle', 'circle-radius', 8);
			map.setPaintProperty('stops-circle', 'circle-color', '#ef4444');
			// Hide all accuracy circles when nothing selected
			map.setFilter('stops-accuracy-circles', ['==', ['get', 'index'], -1]);
		}
	});

	onDestroy(() => {
		if (map) {
			map.remove();
			map = null;
		}
		if (handleKeyDownRef) {
			document.removeEventListener('keydown', handleKeyDownRef);
			handleKeyDownRef = null;
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
