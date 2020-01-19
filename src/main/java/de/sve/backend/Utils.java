package de.sve.backend;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;

public class Utils {

	public static Gson gson() {
		return new GsonBuilder()/*.registerTypeAdapter(DateTime.class, new DateTimeConverter())*/.create();
	}

}
