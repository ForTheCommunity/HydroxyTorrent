import 'package:flutter/material.dart';
import 'package:hydroxy_rc/ui/screens/initial_screen.dart';
import 'package:hydroxy_rc/ui/screens/main_screen.dart';

import 'routes_name.dart';

class Routes {
  static Route<dynamic> generateRoute(RouteSettings settings) {
    switch (settings.name) {
      case RoutesName.initialScreen:
        final statusMessage = settings.arguments as String?;
        return MaterialPageRoute(
          builder: (context) => InitialScreen(statusMessage: statusMessage),
        );

      case RoutesName.mainScreen:
        return MaterialPageRoute(builder: (context) => const MainScreen());

      default:
        return MaterialPageRoute(
          builder: (context) {
            return const Scaffold(
              body: Center(child: Text("No Route Found..")),
            );
          },
        );
    }
  }
}
