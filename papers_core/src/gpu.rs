// GPU and CUDA detection for NVIDIA hardware optimization
// Detects CUDA availability, GPU memory, and device capabilities

use std::process::Command;

/// GPU device information and capabilities.
#[derive(Debug, Clone)]
pub struct GpuDevice {
    pub id: usize,
    pub name: String,
    pub memory_total: usize,
    pub memory_available: usize,
    pub compute_capability: String,
    pub cuda_version: String,
    pub driver_version: String,
    pub is_nvidia: bool,
    pub is_amd: bool,
    pub is_intel: bool,
}

/// GPU and CUDA detection engine.
pub struct GpuDetector {
    devices: Vec<GpuDevice>,
    cuda_available: bool,
    cuda_path: Option<String>,
    driver_version: Option<String>,
}

impl GpuDetector {
    /// Detect available NVIDIA CUDA devices and their capabilities.
    pub fn detect_nvidia_cuda() -> Result<Self, String> {
        let mut detector = Self {
            devices: Vec::new(),
            cuda_available: false,
            cuda_path: None,
            driver_version: None,
        };

        // Check if CUDA is available
        if !detector.check_cuda_availability() {
            return Ok(detector);
        }

        // Detect NVIDIA GPUs
        detector.detect_nvidia_gpus()?;

        // Get CUDA version
        if let Some(ref cuda_path) = detector.cuda_path.clone() {
            detector.detect_cuda_version(cuda_path.as_str())?;
        }

        // Get driver version
        detector.detect_driver_version()?;

        Ok(detector)
    }

    /// Check if CUDA is available on the system.
    fn check_cuda_availability(&mut self) -> bool {
        // Check for nvcc compiler
        if let Ok(output) = Command::new("nvcc").arg("--version").output() {
            if output.status.success() {
                self.cuda_available = true;
                // Try to find nvcc path
                if let Ok(path) = Command::new("which").arg("nvcc").output() {
                    if path.status.success() {
                        self.cuda_path = Some(
                            String::from_utf8_lossy(&path.stdout).trim().to_string()
                        );
                    }
                }
                return true;
            }
        }

        // Check for CUDA runtime libraries
        let lib_paths = &[
            "/usr/local/cuda/lib64/libcuda.so",
            "/usr/local/cuda-12.0/lib64/libcuda.so",
            "/usr/local/cuda-11.8/lib64/libcuda.so",
            "/opt/cuda/lib64/libcuda.so",
        ];
        for lib_path in lib_paths {
            if std::path::Path::new(lib_path).exists() {
                self.cuda_available = true;
                // Extract the base path (remove /lib64/libcuda.so)
                if let Some(base) = lib_path.strip_suffix("/lib64/libcuda.so") {
                    self.cuda_path = Some(base.to_string());
                }
                return true;
            }
        }

        false
    }

    /// Detect NVIDIA GPUs and their properties.
    fn detect_nvidia_gpus(&mut self) -> Result<(), String> {
        // Use nvidia-smi if available
        if let Ok(output) = Command::new("nvidia-smi").arg("--list-gpus").output() {
            if output.status.success() {
                let output_str = String::from_utf8_lossy(&output.stdout);
                self.parse_nvidia_smi_output(&output_str);
                return Ok(());
            }
        }

        // Fallback: check /proc/pci for NVIDIA devices
        self.detect_nvidia_from_pci()?;

        // If no NVIDIA GPUs found, try AMD
        self.detect_amd_gpus()?;

        Ok(())
    }

    /// Parse nvidia-smi output to extract GPU information.
    fn parse_nvidia_smi_output(&mut self, output: &str) {
        let lines: Vec<&str> = output.lines().collect();
        let mut current_device: Option<&str> = None;

        for line in lines {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            // Check if this line is a GPU name
            if line.contains("GPU ") && line.contains("  ") {
                // Extract GPU ID from previous lines if any
                if let Some(id_str) = line.split_whitespace().next() {
                    if id_str.chars().next().unwrap_or(' ').is_ascii_digit() {
                        current_device = Some(line);
                    }
                }
            } else if current_device.is_some() && line.contains("CUDA Version:") {
                // This is a GPU info line, parse it
                self.parse_gpu_info_line(current_device.unwrap(), line);
                current_device = None;
            }
        }
    }

    /// Parse a single GPU info line from nvidia-smi.
    fn parse_gpu_info_line(&mut self, gpu_name: &str, info_line: &str) {
        let mut device = GpuDevice {
            id: 0, // Will be set later
            name: gpu_name.trim().to_string(),
            memory_total: 0,
            memory_available: 0,
            compute_capability: String::new(),
            cuda_version: String::new(),
            driver_version: String::new(),
            is_nvidia: gpu_name.to_lowercase().contains("nvidia"),
            is_amd: gpu_name.to_lowercase().contains("amd") || gpu_name.to_lowercase().contains("radeon"),
            is_intel: gpu_name.to_lowercase().contains("intel"),
        };

        // Parse CUDA version
        if let Some(cuda_part) = info_line.split("CUDA Version:").nth(1) {
            if let Some(cuda_version) = cuda_part.split(",").next() {
                device.cuda_version = cuda_version.trim().to_string();
            }
        }

        // Parse compute capability
        if let Some(cb_part) = info_line.split("Compute Capability:").nth(1) {
            if let Some(cb) = cb_part.split(",").next() {
                device.compute_capability = cb.trim().to_string();
            }
        }

        // Set GPU ID based on position in the list
        device.id = self.devices.len();
        self.devices.push(device);
    }

    /// Detect NVIDIA GPUs from PCI information.
    fn detect_nvidia_from_pci(&mut self) -> Result<(), String> {
        let pci_path = "/sys/class/pci".to_string();
        if !std::path::Path::new(&pci_path).exists() {
            return Ok(());
        }

        let entries = match std::fs::read_dir(&pci_path) {
            Ok(entries) => entries,
            Err(_) => return Ok(()),
        };

        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                let _device_name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                // Check for NVIDIA vendor ID (0x10de)
                let vendor_file = path.join("vendor");
                if let Ok(vendor_content) = std::fs::read_to_string(vendor_file) {
                    let vendor = u32::from_str_radix(vendor_content.trim(), 16).unwrap_or(0);
                    if vendor == 0x10de {
                        // NVIDIA GPU found
                        let device = GpuDevice {
                            id: self.devices.len(),
                            name: Self::get_device_name(&path),
                            memory_total: Self::get_gpu_memory(&path),
                            memory_available: Self::get_gpu_memory(&path),
                            compute_capability: String::new(),
                            cuda_version: String::new(),
                            driver_version: String::new(),
                            is_nvidia: true,
                            is_amd: false,
                            is_intel: false,
                        };
                        self.devices.push(device);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get device name from PCI path.
    fn get_device_name(pci_path: &std::path::Path) -> String {
        let class_file = pci_path.join("class");
        if let Ok(class_content) = std::fs::read_to_string(class_file) {
            let class = u32::from_str_radix(class_content.trim(), 16).unwrap_or(0);
            let device_class = (class >> 8) & 0xff;

            // Check for NVIDIA or AMD GPU classes
            if device_class == 0x03 || device_class == 0x04 {
                // Display controller or Multimedia controller
                // Try to get more specific info
                let subsystem_file = pci_path.join("subsystem_vendor");
                if let Ok(subsystem_content) = std::fs::read_to_string(subsystem_file) {
                    let subsystem = u32::from_str_radix(subsystem_content.trim(), 16).unwrap_or(0);
                    if subsystem == 0x10de {
                        return "NVIDIA GPU".to_string();
                    }
                }

                return "Generic GPU".to_string();
            }
        }

        "Unknown GPU".to_string()
    }

    /// Get GPU memory information from PCI path.
    fn get_gpu_memory(pci_path: &std::path::Path) -> usize {
        let resource_file = pci_path.join("resource");
        if let Ok(resource_content) = std::fs::read_to_string(resource_file) {
            for line in resource_content.lines() {
                if line.contains("memory@") {
                    let parts: Vec<&str> = line.split("@").collect();
                    if parts.len() >= 2 {
                        let memory_part = parts[1].split_whitespace().next().unwrap_or("");
                        if let Some(size_str) = memory_part.strip_suffix("G") {
                            if let Ok(size) = size_str.parse::<f64>() {
                                return (size * 1024.0) as usize;
                            }
                        } else if let Some(size_str) = memory_part.strip_suffix("M") {
                            if let Ok(size) = size_str.parse::<usize>() {
                                return size;
                            }
                        }
                    }
                }
            }
        }
        0
    }

    /// Detect AMD GPUs from PCI information.
    fn detect_amd_gpus(&mut self) -> Result<(), String> {
        let pci_path = "/sys/class/pci".to_string();
        if !std::path::Path::new(&pci_path).exists() {
            return Ok(());
        }

        let entries = match std::fs::read_dir(&pci_path) {
            Ok(entries) => entries,
            Err(_) => return Ok(()),
        };

        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if path.is_dir() {
                let vendor_file = path.join("vendor");
                if let Ok(vendor_content) = std::fs::read_to_string(vendor_file) {
                    let vendor = u32::from_str_radix(vendor_content.trim(), 16).unwrap_or(0);
                    if vendor == 0x1002 {
                        // AMD GPU found
                        let device = GpuDevice {
                            id: self.devices.len(),
                            name: Self::get_device_name(&path),
                            memory_total: Self::get_gpu_memory(&path),
                            memory_available: Self::get_gpu_memory(&path),
                            compute_capability: String::new(),
                            cuda_version: String::new(),
                            driver_version: String::new(),
                            is_nvidia: false,
                            is_amd: true,
                            is_intel: false,
                        };
                        self.devices.push(device);
                    }
                }
            }
        }

        Ok(())
    }

    /// Detect CUDA version from path (no-op — version stored per-device).
    fn detect_cuda_version(&mut self, _cuda_path: &str) -> Result<(), String> {
        // Version is read from nvidia-smi output per device
        Ok(())
    }

    /// Detect driver version.
    fn detect_driver_version(&mut self) -> Result<(), String> {
        let nvidia_smi_output = Command::new("nvidia-smi").arg("--version").output();
        if let Ok(output) = nvidia_smi_output {
            if output.status.success() {
                let version_str = String::from_utf8_lossy(&output.stdout);
                if let Some(version_line) = version_str.lines().next() {
                    self.driver_version = Some(version_line.trim().to_string());
                }
            }
        }
        Ok(())
    }

    /// Get best available GPU for a specific task.
    pub fn get_best_gpu(&self, task: GpuTask) -> Option<&GpuDevice> {
        let mut best_device: Option<&GpuDevice> = None;
        let mut best_score = 0.0;

        for device in &self.devices {
            let score = self.calculate_gpu_score(device, task);
            if score > best_score {
                best_score = score;
                best_device = Some(device);
            }
        }

        best_device
    }

    /// Calculate GPU score for a specific task.
    fn calculate_gpu_score(&self, device: &GpuDevice, task: GpuTask) -> f64 {
        let mut score = 0.0;

        match task {
            GpuTask::Inference => {
                // High memory and compute capability
                score += device.memory_total as f64 * 0.001;
                score += match device.compute_capability.as_str() {
                    "8.0" | "8.6" | "8.9" => 10.0,
                    "7.5" | "7.0" => 8.0,
                    "6.0" | "6.1" => 6.0,
                    _ => 4.0,
                };
                if device.is_nvidia {
                    score += 5.0; // NVIDIA has better CUDA support
                }
            }
            GpuTask::Training => {
                // Very high memory and compute capability
                score += device.memory_total as f64 * 0.002;
                score += match device.compute_capability.as_str() {
                    "8.0" | "8.6" | "8.9" => 15.0,
                    "7.5" | "7.0" => 12.0,
                    "6.0" | "6.1" => 10.0,
                    _ => 6.0,
                };
                if device.is_nvidia {
                    score += 10.0;
                }
            }
            GpuTask::Embedding => {
                // Good balance of memory and compute
                score += device.memory_total as f64 * 0.0015;
                score += match device.compute_capability.as_str() {
                    "8.0" | "8.6" | "8.9" => 8.0,
                    "7.5" | "7.0" => 7.0,
                    "6.0" | "6.1" => 6.0,
                    _ => 4.0,
                };
                if device.is_nvidia {
                    score += 3.0;
                }
            }
            GpuTask::General => {
                // General purpose scoring
                score += device.memory_total as f64 * 0.0005;
                score += 5.0;
            }
        }

        // Adjust for memory availability
        let memory_ratio = device.memory_available as f64 / device.memory_total.max(1) as f64;
        score *= 1.0 + memory_ratio * 0.2;

        score
    }

    /// Check if CUDA is available.
    pub fn is_cuda_available(&self) -> bool {
        self.cuda_available
    }

    /// Get all detected GPU devices.
    pub fn get_devices(&self) -> &[GpuDevice] {
        &self.devices
    }

    /// Get GPU memory in human-readable format.
    pub fn format_memory(memory_mb: usize) -> String {
        if memory_mb >= 1024 * 1024 {
            format!("{:.1} TB", memory_mb as f64 / (1024.0 * 1024.0))
        } else if memory_mb >= 1024 {
            format!("{:.1} GB", memory_mb as f64 / 1024.0)
        } else {
            format!("{:.1} MB", memory_mb as f64)
        }
    }
}

/// GPU tasks for optimization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuTask {
    /// Neural network inference tasks
    Inference,
    /// Neural network training tasks
    Training,
    /// Text embedding generation
    Embedding,
    /// General purpose computation
    General,
}

/// Factory for creating GPU detectors with different configurations.
pub struct GpuDetectorFactory;

impl GpuDetectorFactory {
    /// Create a new GPU detector with NVIDIA-only detection.
    pub fn nvidia_only() -> GpuDetector {
        GpuDetector::new().expect("Failed to create NVIDIA-only GPU detector")
    }

    /// Create a new GPU detector with automatic detection.
    pub fn auto() -> GpuDetector {
        GpuDetector::new().expect("Failed to create auto GPU detector")
    }

    /// Create a new GPU detector for CPU-only systems.
    pub fn cpu_only() -> GpuDetector {
        let mut detector = GpuDetector::new().expect("Failed to create CPU-only GPU detector");
        detector.cuda_available = false;
        detector
    }
}

impl GpuDetector {
    /// Create a new GPU detector.
    fn new() -> Result<Self, String> {
        let mut detector = Self {
            devices: Vec::new(),
            cuda_available: false,
            cuda_path: None,
            driver_version: None,
        };

        // Try to detect NVIDIA CUDA devices
        if let Ok(temp_detector) = GpuDetector::detect_nvidia_cuda() {
            detector = temp_detector;
        }

        Ok(detector)
    }
}