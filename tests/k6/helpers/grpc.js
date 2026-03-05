import grpc from 'k6/net/grpc';
import { GRPC_HOST } from './config.js';

const client = new grpc.Client();
client.load(['../../proto'], 'user.proto');
client.load(['../../proto'], 'map.proto');
client.load(['../../proto'], 'room.proto');
client.load(['../../proto'], 'avatar.proto');
client.load(['../../proto'], 'play.proto');

export function connect() {
  client.connect(GRPC_HOST, { plaintext: true });
}

export function close() {
  client.close();
}

export { client };

export const SVC = {
  USER: {
    LOGIN: 'marble.user.UserService/Login',
    GET_USER: 'marble.user.UserService/GetUser',
    GET_USERS: 'marble.user.UserService/GetUsers',
    UPDATE_PROFILE: 'marble.user.UserService/UpdateProfile',
  },
  MAP: {
    CREATE: 'marble.map.MapService/CreateMap',
    GET: 'marble.map.MapService/GetMap',
    UPDATE: 'marble.map.MapService/UpdateMap',
    DELETE: 'marble.map.MapService/DeleteMap',
    LIST: 'marble.map.MapService/ListMaps',
  },
  ROOM: {
    CREATE: 'marble.room.RoomService/CreateRoom',
    GET: 'marble.room.RoomService/GetRoom',
    LIST: 'marble.room.RoomService/ListRooms',
    JOIN: 'marble.room.RoomService/JoinRoom',
    GET_USERS: 'marble.room.RoomService/GetRoomUsers',
    KICK: 'marble.room.RoomService/KickPlayer',
    START: 'marble.room.RoomService/StartGame',
    REPORT_ARRIVAL: 'marble.room.RoomService/ReportArrival',
    REGISTER_PEER: 'marble.room.RoomService/RegisterPeerId',
    REPORT_CONNECTION: 'marble.room.RoomService/ReportConnection',
    GET_TOPOLOGY: 'marble.room.RoomService/GetTopology',
    GET_ROOM_TOPOLOGY: 'marble.room.RoomService/GetRoomTopology',
    RESOLVE_PEERS: 'marble.room.RoomService/ResolvePeerIds',
  },
  AVATAR: {
    SET: 'marble.avatar.AvatarService/SetAvatar',
    GET: 'marble.avatar.AvatarService/GetAvatar',
    GET_MULTI: 'marble.avatar.AvatarService/GetAvatars',
  },
};
