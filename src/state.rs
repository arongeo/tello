/// Code for parsing the Tello state data into a struct from a string

#[derive(PartialEq)]
pub struct TelloState {
    pub mid: i32,
    pub xyz: [i32; 3],
    pub mpry: [i32; 3],
    pub pitch: i32,
    pub roll: i32,
    pub yaw: i32,
    pub vg: [i32; 3],
    pub temp: [i32; 2],
    pub tof: i32,
    pub h: i32,
    pub bat: u32,
    pub baro: f64,
    pub time: i32,
    pub ag: [f64; 3],
}

impl TelloState {
    /// Parse state data from a string into a TelloState struct
    pub fn new(state_str: &str) -> Self {
        let mut tellostate = Self {
            mid: 0,
            xyz: [0; 3],
            mpry: [0; 3],
            pitch: 0,
            roll: 0,
            yaw: 0,
            vg: [0; 3],
            temp: [0; 2],
            tof: 0,
            h: 0,
            bat: 0,
            baro: 0.0,
            time: 0,
            ag: [0.0; 3],
        };

        let states = state_str.split(";").collect::<Vec<&str>>();

        for (i, state) in states.iter().enumerate() {
            let mut num = state.chars().collect::<Vec<char>>();
            num.retain(|n| (n.is_digit(10)) | (*n == '.') | (*n == ',') | (*n == '-'));

            let str_num = String::from_iter(num);

            match i {
                0 => tellostate.mid = str_num.parse().unwrap(),
                (1..=3) => tellostate.xyz[i-1] = str_num.parse().unwrap(),
                4 => {
                    for (j, val) in str_num.split(",").into_iter().enumerate() {
                        tellostate.mpry[j] = val.parse().unwrap();
                    }
                },
                5 => tellostate.pitch = str_num.parse().unwrap(),
                6 => tellostate.roll = str_num.parse().unwrap(),
                7 => tellostate.yaw = str_num.parse().unwrap(),
                (8..=10) => tellostate.vg[i-8] = str_num.parse().unwrap(),
                (11..=12) => tellostate.temp[i-11] = str_num.parse().unwrap(),
                13 => tellostate.tof = str_num.parse().unwrap(),
                14 => tellostate.h = str_num.parse().unwrap(),
                15 => tellostate.bat = str_num.parse().unwrap(),
                16 => tellostate.baro = str_num.parse().unwrap(),
                17 => tellostate.time = str_num.parse().unwrap(),
                (18..=20) => tellostate.ag[i-18] = str_num.parse().unwrap(),
                _ => {},
            }
        }
        tellostate
    }
}

/// Spawn a thread for parsing state data from the Tello, for telemetry and other purposes
pub fn spawn_state_thread(srx: std::sync::mpsc::Receiver<crate::ThreadMsg>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let state_socket = match crate::UdpSocket::bind("0.0.0.0:8890") {
            Ok(s) => s,
            Err(e) => panic!("ERROR with creating socket: {}", e),
        };

        loop {
            let mut buffer = [0; 2048];
            let msg_len = match state_socket.recv(&mut buffer) {
                Ok(l) => l,
                Err(_) => continue,
            };

            let state_str = match std::str::from_utf8(&buffer[..msg_len]) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let mut tellostate = TelloState::new(state_str);

            match srx.try_recv() {
                Ok(v) => {
                    if v == crate::ThreadMsg::ShutdownThread {
                        break;
                    }
                },
                Err(_) => {},
            };
        }
    })
}
