<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import type { RouteData } from '$lib/types';
	import { getRouteGeometry, getStopPositions } from '$lib/parsers/routeData';
	import { projectCmToLatLon } from '$lib/parsers/projection';
	import 'maplibre-gl/dist/maplibre-gl.css';

	export let routeData: RouteData;
	export let busPosition: { x_cm: number; y_cm: number } | null = null;
	export let selectedStop: number | null = null;
	export let onStopClick: (stopIndex: number) => void = () => {};

	let mapContainer: HTMLDivElement;
	let map: maplibregl.Map | null = null;
	let routeSourceId = 'route';
	let stopsSourceId = 'stops';
	let busSourceId = 'bus';

	onMount(() => {
		// Initialize map
		const [initialLat, initialLon] = [25.0, 121.0]; // Taiwan center

		map = new maplibregl.Map({
			container: mapContainer,
			style: 'https://demotiles.maplibre.org/style.json', // Basic OSM style
			center: [initialLon, initialLat],
			zoom: 13,
			antialias: true
		});

		map.on('load', () => {
			if (!map) return;

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
			const stopFeatures = stops.map((stop) => ({
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
		if (!map || !busPosition) return;

		const [lat, lon] = projectCmToLatLon(busPosition.x_cm, busPosition.y_cm);

		if (map.getSource(busSourceId)) {
			(map.getSource(busSourceId) as maplibregl.GeoJSONSource).setData({
				type: 'Feature',
				properties: {},
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
					properties: {},
					geometry: {
						type: 'Point',
						coordinates: [lon, lat]
					}
				}
			});

			map.addLayer({
				id: 'bus-marker',
				type: 'circle',
				source: busSourceId,
				paint: {
					'circle-radius': 12,
					'circle-color': '#22c55e',
					'circle-stroke-width': 3,
					'circle-stroke-color': '#ffffff'
				}
			});
		}
	});

	// Highlight selected stop
	$effect(() => {
		if (!map) return;

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
