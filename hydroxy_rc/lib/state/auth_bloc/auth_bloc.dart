import 'package:bloc/bloc.dart';
import 'package:freezed_annotation/freezed_annotation.dart';

part 'auth_event.dart';
part 'auth_state.dart';
part 'auth_bloc.freezed.dart';

class AuthBloc extends Bloc<AuthEvent, AuthState> {
  AuthBloc() : super(_Initial()) {
    on<_Login>(_login);
  }

  Future<void> _login(_Login event, Emitter<AuthState> emit) async {
    // trying to do loginn....
    print("Username -> ${event.username}");
    print("Password -> ${event.password}");
    
  }
}
