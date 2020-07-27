package de.sve.backend.model.calendar;

import java.time.LocalDate;
import java.time.LocalDateTime;

import javax.annotation.Nullable;

import com.google.auto.value.AutoValue;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

@AutoValue
@GenerateTypeAdapter
public abstract class Appointment {

	public static Appointment create(String title, String description, LocalDate startDate, LocalDate endDate, LocalDateTime startDateTime, LocalDateTime endDateTime) {
		return new AutoValue_Appointment(title, description, startDate, endDate, startDateTime, endDateTime);
	}

	public abstract String title();

	@Nullable
	public abstract String description();

	@Nullable
	public abstract LocalDate startDate();

	@Nullable
	public abstract LocalDate endDate();

	@Nullable
	public abstract LocalDateTime startDateTime();

	@Nullable
	public abstract LocalDateTime endDateTime();

}
