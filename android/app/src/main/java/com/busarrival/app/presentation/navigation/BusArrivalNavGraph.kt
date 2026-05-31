package com.busarrival.app.presentation.navigation

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.busarrival.app.presentation.ui.config.ConfigScreen
import com.busarrival.app.presentation.ui.detection.DetectionScreen
import com.busarrival.app.presentation.ui.history.HistoryScreen
import com.busarrival.app.presentation.ui.navigation.GlassNavigationBar
import com.busarrival.app.presentation.ui.navigation.NavItem
import com.busarrival.app.presentation.viewmodel.DetectionViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BusArrivalNavGraph() {
    val navController = rememberNavController()
    val navItems = listOf(
        NavItem.Config,
        NavItem.Detection,
        NavItem.History
    )

    val navBackStackEntry by navController.currentBackStackEntryAsState()
    val currentDestination = navBackStackEntry?.destination
    val selectedRoute = currentDestination?.route ?: NavItem.Detection.route

    // Background gradient matching config screen
    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(
                Brush.verticalGradient(
                    colors = listOf(
                        Color(0xFF1A1A2E).copy(alpha = 0.95f),
                        Color(0xFF16213E).copy(alpha = 0.9f),
                        Color(0xFF0F0F1A)
                    )
                )
            )
    ) {
        Scaffold(
            containerColor = Color.Transparent,
            bottomBar = {
                GlassNavigationBar(
                    items = navItems,
                    selectedRoute = selectedRoute,
                    onItemSelected = { route ->
                        navController.navigate(route) {
                            popUpTo(navController.graph.findStartDestination().id) {
                                saveState = true
                            }
                            launchSingleTop = true
                            restoreState = true
                        }
                    }
                )
            }
        ) { innerPadding ->
            NavHost(
                navController = navController,
                startDestination = NavItem.Detection.route,
                modifier = Modifier.padding(innerPadding)
            ) {
                composable(NavItem.Config.route) {
                    ConfigScreen()
                }
                composable(NavItem.Detection.route) {
                    DetectionScreen()
                }
                composable(NavItem.History.route) {
                    HistoryScreen(
                        onSimulateLog = {
                            navController.navigate(NavItem.Detection.route) {
                                popUpTo(navController.graph.findStartDestination().id) {
                                    saveState = true
                                }
                                launchSingleTop = true
                                restoreState = true
                            }
                        }
                    )
                }
            }
        }
    }
}
