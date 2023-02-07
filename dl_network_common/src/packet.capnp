@0xdc945a5579805e11;

struct Ping @0xe2f2b985eeceb168 {
    version       @0 :Text;
    disconnecting @1 :Bool;
}

struct PingResponse @0xe91683d68ad92062 {
    valid   @0 :Bool;
    version @1 :Text;
}

struct LoginRequest @0xe864be34e8f6bf9c {
    username @0 :Text;
    password @1 :Text;
    signup   @2 :Bool;
}

struct LoginResponse @0xeb46a12204d9f07b {
    union {
        valid @0 :Bool;
        error @1 :Text;
    }
}

struct Message @0x871881f4d77e2a9a {
    message   @0 :Text;
    sender    @1 :Text;
    recipient @2 :Text;
    timestamp @3 :Text;
}

struct Error @0x99bc0111f5e2f0fa {
    disconnect @0 :Bool;
    error @1 :Text;
}

struct BigBoiChonk @0x880f3b0abb944bce {
    union {
        message @0 :Message;
        disconnect @1 :Bool;
        error @2 :Error;
    }
}