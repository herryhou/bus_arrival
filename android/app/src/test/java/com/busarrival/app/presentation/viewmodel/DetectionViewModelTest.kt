package com.busarrival.app.presentation.viewmodel

import com.busarrival.app.data.preferences.DetectionPreferences
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import org.junit.After
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment

@RunWith(RobolectricTestRunner::class)
class DetectionViewModelTest {

    private lateinit var preferences: DetectionPreferences

    @Before
    fun setup() {
        preferences = DetectionPreferences(RuntimeEnvironment.getApplication())
        preferences.clear()
    }

    @After
    fun teardown() {
        preferences.clear()
    }

    @Test
    fun cameraFollowEnabledByDefault() {
        val viewModel = DetectionViewModel(RuntimeEnvironment.getApplication())

        assertTrue(viewModel.cameraFollowEnabled.value)
    }

    @Test
    fun toggleCameraFollowFlipsState() {
        val viewModel = DetectionViewModel(RuntimeEnvironment.getApplication())

        viewModel.toggleCameraFollow()

        assertFalse(viewModel.cameraFollowEnabled.value)
    }

    @Test
    fun toggleCameraFollowPersists() {
        val viewModel = DetectionViewModel(RuntimeEnvironment.getApplication())

        viewModel.toggleCameraFollow()

        val reloaded = DetectionViewModel(RuntimeEnvironment.getApplication())
        assertFalse(reloaded.cameraFollowEnabled.value)
    }

    @Test
    fun disableCameraFollowSetsFalse() {
        val viewModel = DetectionViewModel(RuntimeEnvironment.getApplication())

        viewModel.disableCameraFollow()

        assertFalse(viewModel.cameraFollowEnabled.value)
    }

    @Test
    fun disableCameraFollowPersists() {
        val viewModel = DetectionViewModel(RuntimeEnvironment.getApplication())

        viewModel.disableCameraFollow()

        val reloaded = DetectionViewModel(RuntimeEnvironment.getApplication())
        assertFalse(reloaded.cameraFollowEnabled.value)
    }
}
