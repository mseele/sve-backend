package de.sve.backend.model.contact;

import java.util.List;

import com.google.auto.value.AutoValue;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

@AutoValue
@GenerateTypeAdapter
public abstract class Emails {

	public static Emails create(List<Email> list) {
		return new AutoValue_Emails(list);
	}

	public abstract List<Email> emails();

}
