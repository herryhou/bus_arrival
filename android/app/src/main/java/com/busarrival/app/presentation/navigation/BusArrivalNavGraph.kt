package com.busarrival.app.presentation.navigation

import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.List
import androidx.compose.material.icons.filled.Place
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.rounded.*
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.busarrival.app.presentation.navigation.components.GlassBottomBar
import com.busarrival.app.presentation.navigation.components.TabItem
import com.busarrival.app.presentation.ui.config.ConfigScreen
import com.busarrival.app.presentation.ui.detection.DetectionScreen
import com.busarrival.app.presentation.ui.history.HistoryScreen
import com.busarrival.app.presentation.viewmodel.DetectionViewModel

sealed class Screen(val route: String, val title: String) {
    object Config : Screen("config", "Config")
    object Detection : Screen("detection", "Detect")
    object History : Screen("history", "History")
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BusArrivalNavGraph() {
    val navController = rememberNavController()

    val tabs = listOf(
        TabItem(
            route = Screen.Config.route,
            title = Screen.Config.title,
            icon = Icons.Rounded.Tune,
            activeIcon = Icons.Rounded.Tune
        ),
        TabItem(
            route = Screen.Detection.route,
            title = Screen.Detection.title,
            icon = Icons.Rounded.LocationOn,
            activeIcon = Icons.Rounded.LocationOn
        ),
        TabItem(
            route = Screen.History.route,
            title = Screen.History.title,
            icon = Icons.Rounded.History,
            activeIcon = Icons.Rounded.History
        )
    )

    Scaffold(
        bottomBar = {
            val navBackStackEntry by navController.currentBackStackEntryAsState()
            val currentDestination = navBackStackEntry?.destination

            GlassBottomBar(
                tabs = tabs,
                selectedRoute = currentDestination?.route ?: Screen.Detection.route,
                onTabSelected = { route ->
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
            startDestination = Screen.Detection.route,
            modifier = Modifier.padding(
                top = innerPadding.calculateTopPadding(),
                bottom = innerPadding.calculateBottomPadding()
            )
        ) {
            composable(Screen.Config.route) {
                ConfigScreen()
            }
            composable(Screen.Detection.route) {
                DetectionScreen()
            }
            composable(Screen.History.route) {
                HistoryScreen(
                    onSimulateLog = {
                        navController.navigate(Screen.Detection.route) {
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
