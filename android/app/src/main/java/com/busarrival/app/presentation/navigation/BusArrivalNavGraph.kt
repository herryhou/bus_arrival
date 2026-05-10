package com.busarrival.app.presentation.navigation

import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.History
import androidx.compose.material.icons.filled.Map
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.navigation.NavDestination.Companion.hierarchy
import androidx.navigation.NavGraph.Companion.findStartDestination
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.busarrival.app.presentation.ui.config.ConfigScreen
import com.busarrival.app.presentation.ui.detection.DetectionScreen
import com.busarrival.app.presentation.ui.history.HistoryScreen
import com.busarrival.app.presentation.viewmodel.DetectionViewModel

sealed class Screen(val route: String, val title: String, val icon: androidx.compose.ui.graphics.vector.ImageVector) {
    object Config : Screen("config", "Config", Icons.Default.Settings)
    object Detection : Screen("detection", "Detect", Icons.Default.Map)
    object History : Screen("history", "History", Icons.Default.History)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun BusArrivalNavGraph(
    viewModel: DetectionViewModel? = null
) {
    val navController = rememberNavController()
    val uiState by viewModel?.uiState?.collectAsState()

    Scaffold(
        bottomBar = {
            NavigationBar {
                val navBackStackEntry by navController.currentBackStackEntryAsState()
                val currentDestination = navBackStackEntry?.destination

                Screen.values().forEach { screen ->
                    NavigationBarItem(
                        icon = {
                            BadgedBox(
                                badge = {
                                    if (screen == Screen.Detection && uiState?.isRunning == true) {
                                        Badge()
                                    }
                                }
                            ) {
                                Icon(screen.icon, contentDescription = screen.title)
                            }
                        },
                        label = { Text(screen.title) },
                        selected = currentDestination?.hierarchy?.any { it.route == screen.route } == true,
                        onClick = {
                            navController.navigate(screen.route) {
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
    ) { innerPadding ->
        NavHost(
            navController = navController,
            startDestination = Screen.Config.route,  // Start at Config to load route first
            modifier = Modifier.padding(innerPadding)
        ) {
            composable(Screen.Config.route) {
                ConfigScreen()
            }
            composable(Screen.Detection.route) {
                DetectionScreen()
            }
            composable(Screen.History.route) {
                HistoryScreen()
            }
        }
    }
}
