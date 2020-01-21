package de.sve.backend;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

public class Utils {

	public static Gson gson() {
		return new GsonBuilder().registerTypeAdapterFactory(GenerateTypeAdapter.FACTORY)
								/*.registerTypeAdapter(DateTime.class, new DateTimeConverter())*/
								.create();
	}

}
