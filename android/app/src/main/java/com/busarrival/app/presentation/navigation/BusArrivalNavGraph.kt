package com.busarrival.app.presentation.navigation

import androidx.compose.runtime.Composable
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.busarrival.app.presentation.ui.detection.DetectionScreen
import com.busarrival.app.presentation.ui.history.HistoryScreen
import com.busarrival.app.presentation.ui.config.ConfigScreen

sealed class Screen(val route: String) {
    object Detection : Screen("detection")
    object History : Screen("history")
    object Config : Screen("config")
}

@Composable
fun BusArrivalNavGraph(
    navController: NavHostController = rememberNavController()
) {
    NavHost(
        navController = navController,
        startDestination = Screen.Detection.route
    ) {
        composable(Screen.Detection.route) {
            DetectionScreen(navController)
        }
        composable(Screen.History.route) {
            HistoryScreen(navController)
        }
        composable(Screen.Config.route) {
            ConfigScreen(navController)
        }
    }
}
